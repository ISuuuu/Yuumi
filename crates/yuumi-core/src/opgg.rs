//! OP.GG 英雄数据远程查询（梯队榜单 + 符文出装详情）
//!
//! 对照 Vue/Tauri 版的两处实现：`src-tauri/src/lcu/opgg.rs`（缓存、客户端复用与重试）
//! 与 `src-tauri/src/tools.rs::fetch_opgg_data`（路由拼接）。
//! GPUI 侧直接调用本模块，不再经 IPC 转发。
//!
//! 约定：响应的结构化解析与归一化都在本模块完成，UI 只消费归一化后的类型，
//! 避免「缺字段 / 零场次 / snake_case 与 camelCase 混用」这类判断散落在渲染代码里。

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// OP.GG 英雄数据接口根地址
const API_BASE: &str = "https://lol-api-champion.op.gg/api";
/// 内存缓存条数上限
const CACHE_MAX_ENTRIES: usize = 100;
/// 内存缓存有效期（10 分钟）
const CACHE_TTL: Duration = Duration::from_secs(600);
/// 磁盘缓存有效期（24 小时）
const DISK_CACHE_TTL: Duration = Duration::from_secs(86_400);
/// 单次请求超时：境外服务常走代理，避免连接挂起
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// 传输层最大尝试次数
const MAX_ATTEMPTS: u32 = 3;
/// 重试间隔
const RETRY_DELAY: Duration = Duration::from_millis(500);

// ─── 查询参数 ───

/// OP.GG 榜单模式（与 Vue `OpggFilterBar.vue` 的 mode 取值一一对应）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpggMode {
    Ranked,
    Normal,
    Aram,
    Arena,
}

impl OpggMode {
    /// 接口路径段取值
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ranked => "ranked",
            Self::Normal => "normal",
            Self::Aram => "aram",
            Self::Arena => "arena",
        }
    }

    /// 由筛选栏取值反查模式，未知值退回排位
    pub fn from_filter_value(value: &str) -> Self {
        match value {
            "normal" => Self::Normal,
            "aram" => Self::Aram,
            "arena" => Self::Arena,
            _ => Self::Ranked,
        }
    }

    /// 仅排位模式的详情请求携带分路段
    pub fn uses_position(self) -> bool {
        matches!(self, Self::Ranked)
    }
}

/// 访问 OP.GG 的代理设置（来自全局配置 `general` 段）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProxyConfig {
    pub enable: bool,
    pub addr: String,
}

// ─── 归一化输出类型（供 UI 直接消费）──

/// 统计快照：百分比为 0.0~1.0 的原始比例，缺值保持 `None`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    pub win_rate: Option<f64>,
    pub pick_rate: Option<f64>,
    pub ban_rate: Option<f64>,
    pub kda: Option<f64>,
    pub tier: Option<String>,
    pub rank: Option<i32>,
}

/// 梯队榜单条目
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TierListEntry {
    pub champion_id: i32,
    pub name: String,
    pub stats: Stats,
    /// 克制英雄 ID（最多三个由 UI 截取）
    pub counters: Vec<i32>,
}

/// 梯队榜单结果
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TierList {
    pub version: String,
    pub entries: Vec<TierListEntry>,
}

/// 详情页某个分路的统计（含分路名，用于按位置取数）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PositionStats {
    pub position: String,
    pub stats: Stats,
}

/// 详情页英雄摘要
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildSummary {
    pub champion_id: Option<i32>,
    pub name: Option<String>,
    pub positions: Vec<PositionStats>,
    pub average_stats: Option<Stats>,
}

impl BuildSummary {
    /// 按 Vue 规则取统计：给了分路就优先该分路，否则退回整体统计
    pub fn stats_for(&self, position: Option<&str>) -> Option<&Stats> {
        if let Some(pos) = position {
            if let Some(found) = self.positions.iter().find(|p| p.position == pos) {
                return Some(&found.stats);
            }
        }
        self.average_stats.as_ref()
    }
}

/// 克制/被克条目
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CounterEntry {
    pub champion_id: i32,
    pub name: Option<String>,
    pub rate: Option<f64>,
    pub play: u32,
}

/// 一套符文方案
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunePreset {
    pub primary_page_id: i32,
    pub secondary_page_id: i32,
    pub primary_rune_ids: Vec<i32>,
    pub secondary_rune_ids: Vec<i32>,
    pub stat_mod_ids: Vec<i32>,
    pub win_rate: Option<f64>,
    pub play: u32,
    pub pick_rate: Option<f64>,
}

/// 一组「ID + 胜率」数据（召唤师技能、装备共用）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IdWinRate {
    pub ids: Vec<i32>,
    pub win_rate: Option<f64>,
    pub play: u32,
    pub pick_rate: Option<f64>,
}

/// 技能加点顺序
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillOrder {
    /// 升级顺序，例如 ["Q", "W", "E", "Q", ...]
    pub order: Vec<String>,
    pub win_rate: Option<f64>,
    pub play: u32,
    pub pick_rate: Option<f64>,
}

/// 主加技能优先级，例如 ["Q", "E", "W"]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillMastery {
    pub ids: Vec<String>,
}

/// 符文出装详情
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChampionBuild {
    pub version: String,
    pub summary: Option<BuildSummary>,
    pub counters: Vec<CounterEntry>,
    pub runes: Vec<RunePreset>,
    pub skills: Vec<SkillOrder>,
    pub skill_masteries: Vec<SkillMastery>,
    pub summoner_spells: Vec<IdWinRate>,
    pub starter_items: Vec<IdWinRate>,
    pub core_items: Vec<IdWinRate>,
    pub boots: Vec<IdWinRate>,
    pub last_items: Vec<IdWinRate>,
}

impl ChampionBuild {
    /// 克制方（胜率 >= 50%），按胜率降序取前五
    pub fn strong_counters(&self) -> Vec<&CounterEntry> {
        let mut list: Vec<&CounterEntry> = self
            .counters
            .iter()
            .filter(|c| c.rate.is_some_and(|r| r >= 0.5))
            .collect();
        list.sort_by(|a, b| {
            b.rate
                .unwrap_or_default()
                .partial_cmp(&a.rate.unwrap_or_default())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list.truncate(5);
        list
    }

    /// 被克方（胜率 < 50%），按胜率升序取前五
    pub fn weak_counters(&self) -> Vec<&CounterEntry> {
        let mut list: Vec<&CounterEntry> = self
            .counters
            .iter()
            .filter(|c| c.rate.is_some_and(|r| r < 0.5))
            .collect();
        list.sort_by(|a, b| {
            a.rate
                .unwrap_or_default()
                .partial_cmp(&b.rate.unwrap_or_default())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list.truncate(5);
        list
    }

    /// 是否存在任何装备数据（决定「一键应用出装」按钮显隐）
    pub fn has_item_data(&self) -> bool {
        !self.starter_items.is_empty()
            || !self.core_items.is_empty()
            || !self.boots.is_empty()
            || !self.last_items.is_empty()
    }
}

// ─── 宽松数值解析 ───

/// 兼容数字、数字字符串（含百分号）与对象形态的数值字段。
///
/// OP.GG 同一字段在不同区服/模式下可能返回 `0.5234`、`"0.5234"`，
/// 部分统计位会返回对象结构；无法识别时按缺失处理，绝不让整个响应解析失败。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct FlexNumber(Option<f64>);

impl<'de> Deserialize<'de> for FlexNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<Value>::deserialize(deserializer)?;
        Ok(Self(match value {
            Some(Value::Number(number)) => number.as_f64(),
            Some(Value::String(text)) => text
                .trim()
                .trim_end_matches('%')
                .parse::<f64>()
                .ok(),
            _ => None,
        }))
    }
}

impl FlexNumber {
    /// 转为有符号整数（场次、排名等），小数按截断处理
    fn as_i32(self) -> Option<i32> {
        self.0.map(|v| v as i32)
    }

    /// 转为非负场次，缺失或负值按 0
    fn as_count(self) -> u32 {
        self.0.filter(|v| *v > 0.0).map(|v| v as u32).unwrap_or(0)
    }

    fn value(self) -> Option<f64> {
        self.0
    }
}

/// 由胜场与场次推导胜率，场次为 0 时返回 `None`（零场次按缺失展示）
fn win_rate_of(win: FlexNumber, play: FlexNumber) -> Option<f64> {
    let play = play.0?;
    if play <= 0.0 {
        return None;
    }
    win.0.map(|w| w / play)
}

/// Tier 值：接口可能返回数字（1）或字符串（"1" / "T1"）
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
enum TierValue {
    Number(f64),
    Text(String),
}

impl TierValue {
    fn display(&self) -> String {
        match self {
            Self::Number(n) if n.fract() == 0.0 => format!("{}", *n as i64),
            Self::Number(n) => format!("{n}"),
            Self::Text(text) => text.trim().to_string(),
        }
    }
}

// ─── 原始响应结构 ───

#[derive(Debug, Default, Deserialize)]
struct RawMeta {
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawStats {
    #[serde(default, alias = "winRate")]
    win_rate: FlexNumber,
    #[serde(default, alias = "pickRate")]
    pick_rate: FlexNumber,
    #[serde(default, alias = "banRate")]
    ban_rate: FlexNumber,
    #[serde(default)]
    kda: FlexNumber,
    #[serde(default)]
    tier: Option<TierValue>,
    #[serde(default)]
    rank: FlexNumber,
    #[serde(default, alias = "tierData")]
    tier_data: Option<RawTierData>,
}

#[derive(Debug, Default, Deserialize)]
struct RawTierData {
    #[serde(default)]
    tier: Option<TierValue>,
    #[serde(default)]
    rank: FlexNumber,
}

impl RawStats {
    fn normalize(&self) -> Stats {
        // tier_data 优先：排位分路统计的真实分级放在这里
        let tier = self
            .tier_data
            .as_ref()
            .and_then(|data| data.tier.as_ref())
            .or(self.tier.as_ref())
            .map(TierValue::display);
        let rank = self
            .tier_data
            .as_ref()
            .and_then(|data| data.rank.as_i32())
            .or_else(|| self.rank.as_i32());
        Stats {
            win_rate: self.win_rate.value(),
            pick_rate: self.pick_rate.value(),
            ban_rate: self.ban_rate.value(),
            kda: self.kda.value(),
            tier,
            rank,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawCounterRef {
    #[serde(default, alias = "championId")]
    champion_id: Option<i32>,
}

#[derive(Debug, Default, Deserialize)]
struct RawPosition {
    #[serde(default)]
    name: String,
    #[serde(default)]
    stats: Option<RawStats>,
    #[serde(default)]
    counters: Vec<RawCounterRef>,
}

#[derive(Debug, Default, Deserialize)]
struct RawChampion {
    #[serde(default)]
    id: i32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    positions: Vec<RawPosition>,
    #[serde(default)]
    average_stats: Option<RawStats>,
}

#[derive(Debug, Default, Deserialize)]
struct RawTierList {
    #[serde(default)]
    meta: Option<RawMeta>,
    #[serde(default)]
    data: Vec<RawChampion>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCounter {
    #[serde(default, alias = "championId")]
    champion_id: i32,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    play: FlexNumber,
    #[serde(default)]
    win: FlexNumber,
}

#[derive(Debug, Default, Deserialize)]
struct RawRunePreset {
    #[serde(default)]
    primary_page_id: i32,
    #[serde(default)]
    secondary_page_id: i32,
    #[serde(default)]
    primary_rune_ids: Vec<i32>,
    #[serde(default)]
    secondary_rune_ids: Vec<i32>,
    #[serde(default)]
    stat_mod_ids: Vec<i32>,
    #[serde(default)]
    play: FlexNumber,
    #[serde(default)]
    win: FlexNumber,
    #[serde(default)]
    pick_rate: FlexNumber,
}

#[derive(Debug, Default, Deserialize)]
struct RawIdWinRate {
    #[serde(default)]
    ids: Vec<i32>,
    #[serde(default)]
    play: FlexNumber,
    #[serde(default)]
    win: FlexNumber,
    #[serde(default)]
    pick_rate: FlexNumber,
}

#[derive(Debug, Default, Deserialize)]
struct RawSkillOrder {
    #[serde(default)]
    order: Vec<String>,
    #[serde(default)]
    play: FlexNumber,
    #[serde(default)]
    win: FlexNumber,
    #[serde(default)]
    pick_rate: FlexNumber,
}

#[derive(Debug, Default, Deserialize)]
struct RawSkillMastery {
    #[serde(default)]
    ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawSummary {
    #[serde(default)]
    id: Option<i32>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    positions: Vec<RawPosition>,
    #[serde(default)]
    average_stats: Option<RawStats>,
}

#[derive(Debug, Default, Deserialize)]
struct RawBuild {
    #[serde(default)]
    summary: Option<RawSummary>,
    #[serde(default)]
    counters: Vec<RawCounter>,
    #[serde(default)]
    runes: Vec<RawRunePreset>,
    #[serde(default)]
    skills: Vec<RawSkillOrder>,
    #[serde(default, alias = "skillMasteries")]
    skill_masteries: Vec<RawSkillMastery>,
    #[serde(default)]
    summoner_spells: Vec<RawIdWinRate>,
    #[serde(default)]
    starter_items: Vec<RawIdWinRate>,
    #[serde(default)]
    core_items: Vec<RawIdWinRate>,
    #[serde(default)]
    boots: Vec<RawIdWinRate>,
    #[serde(default)]
    last_items: Vec<RawIdWinRate>,
}

#[derive(Debug, Default, Deserialize)]
struct RawBuildPayload {
    #[serde(default)]
    meta: Option<RawMeta>,
    #[serde(default)]
    data: Option<RawBuild>,
}

// ─── 归一化实现 ───

impl RawTierList {
    /// 归一化榜单：排位按分路统计，其余模式取 `average_stats`，两者都按 rank 升序
    fn normalize(self, mode: OpggMode, position: Option<&str>) -> TierList {
        let mut entries: Vec<TierListEntry> = if mode.uses_position() {
            let target = position.unwrap_or("MID");
            self.data
                .into_iter()
                .filter_map(|champion| {
                    let position_entry = champion
                        .positions
                        .into_iter()
                        .find(|p| p.name.eq_ignore_ascii_case(target))?;
                    let stats = position_entry.stats.map(|s| s.normalize()).unwrap_or_default();
                    Some(TierListEntry {
                        champion_id: champion.id,
                        name: champion.name,
                        stats,
                        counters: position_entry
                            .counters
                            .into_iter()
                            .filter_map(|c| c.champion_id)
                            .collect(),
                    })
                })
                .collect()
        } else {
            self.data
                .into_iter()
                .filter_map(|champion| {
                    let stats = champion.average_stats?.normalize();
                    // 非排位模式要求整体统计具备排名，缺失即不参与排序展示
                    stats.rank?;
                    Some(TierListEntry {
                        champion_id: champion.id,
                        name: champion.name,
                        stats,
                        counters: Vec::new(),
                    })
                })
                .collect()
        };

        entries.sort_by_key(|entry| entry.stats.rank.unwrap_or(999));

        TierList {
            version: self
                .meta
                .and_then(|meta| meta.version)
                .unwrap_or_default(),
            entries,
        }
    }
}

impl RawBuild {
    fn normalize(self, version: String) -> ChampionBuild {
        ChampionBuild {
            version,
            summary: self.summary.map(|summary| BuildSummary {
                champion_id: summary.id,
                name: summary.name,
                positions: summary
                    .positions
                    .into_iter()
                    .map(|entry| PositionStats {
                        stats: entry.stats.map(|s| s.normalize()).unwrap_or_default(),
                        position: entry.name,
                    })
                    .collect(),
                average_stats: summary.average_stats.map(|s| s.normalize()),
            }),
            counters: self
                .counters
                .into_iter()
                .map(|counter| CounterEntry {
                    champion_id: counter.champion_id,
                    name: counter.name,
                    rate: win_rate_of(counter.win, counter.play),
                    play: counter.play.as_count(),
                })
                .collect(),
            runes: self
                .runes
                .into_iter()
                .map(|rune| RunePreset {
                    primary_page_id: rune.primary_page_id,
                    secondary_page_id: rune.secondary_page_id,
                    primary_rune_ids: rune.primary_rune_ids,
                    secondary_rune_ids: rune.secondary_rune_ids,
                    stat_mod_ids: rune.stat_mod_ids,
                    win_rate: win_rate_of(rune.win, rune.play),
                    play: rune.play.as_count(),
                    pick_rate: rune.pick_rate.value(),
                })
                .collect(),
            skills: self
                .skills
                .into_iter()
                .map(|skill| SkillOrder {
                    order: skill.order,
                    win_rate: win_rate_of(skill.win, skill.play),
                    play: skill.play.as_count(),
                    pick_rate: skill.pick_rate.value(),
                })
                .collect(),
            skill_masteries: self
                .skill_masteries
                .into_iter()
                .map(|mastery| SkillMastery { ids: mastery.ids })
                .collect(),
            summoner_spells: self.summoner_spells.into_iter().map(normalize_id_win_rate).collect(),
            starter_items: self.starter_items.into_iter().map(normalize_id_win_rate).collect(),
            core_items: self.core_items.into_iter().map(normalize_id_win_rate).collect(),
            boots: self.boots.into_iter().map(normalize_id_win_rate).collect(),
            last_items: self.last_items.into_iter().map(normalize_id_win_rate).collect(),
        }
    }
}

fn normalize_id_win_rate(raw: RawIdWinRate) -> IdWinRate {
    IdWinRate {
        ids: raw.ids,
        win_rate: win_rate_of(raw.win, raw.play),
        play: raw.play.as_count(),
        pick_rate: raw.pick_rate.value(),
    }
}

// ─── 缓存 ───

struct CacheEntry {
    payload: Value,
    inserted_at: Instant,
}

static MEMORY_CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();

fn memory_cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    MEMORY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// FNV-1a 64 位稳定哈希，生成磁盘缓存文件名（与 `lcu::client::stable_hash` 同源算法）
fn cache_file_name(key: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}.json")
}

fn disk_cache_dir() -> Option<std::path::PathBuf> {
    let dir = crate::runtime::app_data_dir().join("cache").join("opgg");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// 内存缓存读取（未过期才返回）
fn memory_get(key: &str) -> Option<Value> {
    let cache = memory_cache().lock().ok()?;
    let entry = cache.get(key)?;
    if entry.inserted_at.elapsed() < CACHE_TTL {
        Some(entry.payload.clone())
    } else {
        None
    }
}

/// 内存缓存写入（超上限先淘汰最旧条目）
fn memory_put(key: String, payload: Value) {
    let Ok(mut cache) = memory_cache().lock() else {
        return;
    };
    if cache.len() >= CACHE_MAX_ENTRIES {
        if let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.inserted_at)
            .map(|(key, _)| key.clone())
        {
            cache.remove(&oldest);
        }
    }
    cache.insert(
        key,
        CacheEntry {
            payload,
            inserted_at: Instant::now(),
        },
    );
}

/// 磁盘缓存读取（过期文件顺手删除）
async fn disk_get(key: &str) -> Option<Value> {
    let key_owned = key.to_string();
    tokio::task::spawn_blocking(move || {
        let dir = disk_cache_dir()?;
        let path = dir.join(cache_file_name(&key_owned));
        let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
        if modified.elapsed().unwrap_or(Duration::MAX) > DISK_CACHE_TTL {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&text).ok()
    })
    .await
    .ok()
    .flatten()
}

/// 磁盘缓存写入：独立线程原子写（tmp + rename），不依赖 Tokio 运行时
fn disk_put(key: &str, payload: &Value) {
    let key_owned = key.to_string();
    let Ok(text) = serde_json::to_string(payload) else {
        return;
    };
    std::thread::spawn(move || {
        let Some(dir) = disk_cache_dir() else {
            return;
        };
        let name = cache_file_name(&key_owned);
        let target = dir.join(&name);
        let temp = dir.join(format!("{name}.tmp"));
        if std::fs::write(&temp, text).is_ok() {
            let _ = std::fs::rename(&temp, &target);
        }
    });
}

async fn cached_get(key: &str) -> Option<Value> {
    if let Some(payload) = memory_get(key) {
        return Some(payload);
    }
    let payload = disk_get(key).await?;
    memory_put(key.to_string(), payload.clone());
    Some(payload)
}

/// 仅缓存 HTTP 成功且通过结构校验的响应
fn cached_put(key: &str, payload: Value) {
    memory_put(key.to_string(), payload.clone());
    disk_put(key, &payload);
}

// ─── HTTP 客户端 ───

struct ClientEntry {
    proxy: ProxyConfig,
    client: reqwest::Client,
}

static CLIENT: OnceLock<Mutex<Option<ClientEntry>>> = OnceLock::new();

fn build_client(proxy: &ProxyConfig) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        );
    if proxy.enable && !proxy.addr.is_empty() {
        let url = if proxy.addr.contains("://") {
            proxy.addr.clone()
        } else {
            format!("http://{}", proxy.addr)
        };
        match reqwest::Proxy::all(&url) {
            Ok(configured) => {
                builder = builder.proxy(configured);
                log::info!("[OPGG] 请求已配置代理: {url}");
            }
            Err(err) => log::warn!("[OPGG] 代理地址无效 {}: {err}", proxy.addr),
        }
    }
    builder.build().unwrap_or_default()
}

/// 取进程级复用的客户端，代理配置变化时重建
fn client_for(proxy: &ProxyConfig) -> reqwest::Client {
    let slot = CLIENT.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = slot.lock() else {
        return build_client(proxy);
    };
    if let Some(entry) = guard.as_ref() {
        if &entry.proxy == proxy {
            return entry.client.clone();
        }
    }
    let client = build_client(proxy);
    *guard = Some(ClientEntry {
        proxy: proxy.clone(),
        client: client.clone(),
    });
    client
}

/// 代理抓取 GitHub 文本资源（公告、版本信息等）
pub async fn fetch_github_text(url: &str, proxy: &ProxyConfig) -> Result<String, String> {
    let client = client_for(proxy);
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP 状态码: {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

/// 带缓存与传输层重试的 GET + 类型化校验
async fn get_typed<T>(
    url: &str,
    query: &[(&str, &str)],
    cache_key: &str,
    proxy: &ProxyConfig,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    if let Some(payload) = cached_get(cache_key).await {
        // 缓存内容可能因结构变更而失效，解析失败时按未命中处理，避免永久卡死
        match serde_json::from_value::<T>(payload) {
            Ok(typed) => return Ok(typed),
            Err(err) => log::warn!("[OPGG] 缓存结构已过期，重新请求 {cache_key}: {err}"),
        }
    }

    let client = client_for(proxy);
    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        match client.get(url).query(query).send().await {
            Ok(response) => {
                let status = response.status();
                let text = response
                    .text()
                    .await
                    .map_err(|err| format!("读取 OP.GG 响应失败: {err}"))?;
                if !status.is_success() {
                    last_error = format!("OP.GG 返回 HTTP {status}");
                    log::warn!("[OPGG] {last_error}: {url}");
                } else {
                    match serde_json::from_str::<T>(&text) {
                        Ok(typed) => {
                            // 结构校验通过后才落缓存
                            if let Ok(payload) = serde_json::from_str::<Value>(&text) {
                                cached_put(cache_key, payload);
                            }
                            return Ok(typed);
                        }
                        Err(err) => {
                            return Err(format!("解析 OP.GG 响应失败: {err}"));
                        }
                    }
                }
            }
            Err(err) => {
                last_error = format!("OP.GG 请求失败: {err}");
                log::warn!("[OPGG] {last_error}");
            }
        }

        if attempt < MAX_ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }
    Err(last_error)
}

// ─── 对外查询入口 ───

/// 榜单请求 URL（不带分路段）
pub fn tier_list_url(region: &str, mode: OpggMode) -> String {
    format!("{API_BASE}/{region}/champions/{}", mode.as_str())
}

/// 详情请求 URL：斗魂竞技场走独立路由，其余模式在路径末段带上分路（缺省为 `none`）
pub fn champion_url(
    region: &str,
    mode: OpggMode,
    champion_id: i32,
    position: Option<&str>,
) -> String {
    if mode == OpggMode::Arena {
        return format!("{API_BASE}/{region}/champions/{champion_id}");
    }
    let position_segment = position.unwrap_or("none");
    format!(
        "{API_BASE}/{region}/champions/{}/{}/{position_segment}",
        mode.as_str(),
        champion_id
    )
}

/// 查询英雄梯队榜单
pub async fn fetch_tier_list(
    region: &str,
    mode: OpggMode,
    tier: &str,
    position: Option<&str>,
    proxy: &ProxyConfig,
) -> Result<TierList, String> {
    let url = tier_list_url(region, mode);
    let cache_key = format!("tier:{region}:{}:{tier}", mode.as_str());
    let raw: RawTierList = get_typed(&url, &[("tier", tier)], &cache_key, proxy).await?;
    let list = raw.normalize(mode, position);
    log::info!(
        "[OPGG] 榜单就绪 region={region} mode={} tier={tier} pos={:?} 条目={} version={}",
        mode.as_str(),
        position,
        list.entries.len(),
        list.version
    );
    Ok(list)
}

/// 查询单个英雄的符文出装详情
pub async fn fetch_champion_build(
    region: &str,
    mode: OpggMode,
    tier: &str,
    champion_id: i32,
    position: Option<&str>,
    proxy: &ProxyConfig,
) -> Result<ChampionBuild, String> {
    let url = champion_url(region, mode, champion_id, position);
    let position_segment = position.unwrap_or("none");
    let cache_key = format!(
        "build:{region}:{}:{tier}:{champion_id}:{position_segment}",
        mode.as_str()
    );
    let raw: RawBuildPayload = get_typed(&url, &[("tier", tier)], &cache_key, proxy).await?;
    let version = raw
        .meta
        .and_then(|meta| meta.version)
        .unwrap_or_default();
    let build = raw
        .data
        .map(|data| data.normalize(version))
        .ok_or_else(|| "OP.GG 详情响应缺少 data 字段".to_string())?;
    log::info!(
        "[OPGG] 详情就绪 champion={champion_id} mode={} 符文={} 技能={} 核心装={}",
        mode.as_str(),
        build.runes.len(),
        build.skills.len(),
        build.core_items.len()
    );
    Ok(build)
}

// ─── 符文页与装备页载荷构造（供 yuumi-gpui 写入 LCU 前复用）──

/// 单条装备页条目
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ItemSetEntry {
    pub id: String,
    pub count: u32,
}

/// 装备页分组
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ItemSetBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub items: Vec<ItemSetEntry>,
}

/// 按出现顺序去重追加装备 ID
fn push_unique(ids: &[i32], entries: &mut Vec<ItemSetEntry>, seen: &mut Vec<String>) {
    for id in ids {
        let key = id.to_string();
        if !seen.contains(&key) {
            seen.push(key.clone());
            entries.push(ItemSetEntry {
                id: key,
                count: 1,
            });
        }
    }
}

/// 按 Vue `BuildDetailPanel.vue` 的导入规则构造三块装备页分组。
///
/// 出门装取前两组，核心装取前两组核心装加前两组鞋子并去重，
/// 可选装备取前十条且排除已进入核心装的条目。
pub fn build_item_set_blocks(build: &ChampionBuild) -> Vec<ItemSetBlock> {
    let mut starter_items: Vec<ItemSetEntry> = Vec::new();
    let mut seen_starter: Vec<String> = Vec::new();
    for group in build.starter_items.iter().take(2) {
        for id in &group.ids {
            let key = id.to_string();
            if !seen_starter.contains(&key) {
                seen_starter.push(key.clone());
                starter_items.push(ItemSetEntry {
                    id: key,
                    count: 1,
                });
            }
        }
    }

    let mut core_items: Vec<ItemSetEntry> = Vec::new();
    let mut seen_core: Vec<String> = Vec::new();
    for group in build.core_items.iter().take(2) {
        push_unique(&group.ids, &mut core_items, &mut seen_core);
    }
    for group in build.boots.iter().take(2) {
        push_unique(&group.ids, &mut core_items, &mut seen_core);
    }

    let mut last_items: Vec<ItemSetEntry> = Vec::new();
    let mut seen_last: Vec<String> = Vec::new();
    for group in build.last_items.iter().take(10) {
        for id in &group.ids {
            let key = id.to_string();
            if !seen_core.contains(&key) && !seen_last.contains(&key) {
                seen_last.push(key.clone());
                last_items.push(ItemSetEntry {
                    id: key,
                    count: 1,
                });
            }
        }
    }

    let mut blocks: Vec<ItemSetBlock> = Vec::new();
    if !starter_items.is_empty() {
        blocks.push(ItemSetBlock {
            block_type: "出门装备".to_string(),
            items: starter_items,
        });
    }
    if !core_items.is_empty() {
        blocks.push(ItemSetBlock {
            block_type: "核心装备".to_string(),
            items: core_items,
        });
    }
    if !last_items.is_empty() {
        blocks.push(ItemSetBlock {
            block_type: "可选与推荐装备".to_string(),
            items: last_items,
        });
    }
    blocks
}

/// 符文页写入载荷所需的碎片化 ID 序列：主系 + 副系 + 属性碎片
pub fn rune_perk_ids(preset: &RunePreset) -> Vec<i32> {
    preset
        .primary_rune_ids
        .iter()
        .chain(preset.secondary_rune_ids.iter())
        .chain(preset.stat_mod_ids.iter())
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_tier(json: &str, mode: OpggMode, position: Option<&str>) -> TierList {
        let raw: RawTierList = serde_json::from_str(json).expect("榜单样本应可解析");
        raw.normalize(mode, position)
    }

    #[test]
    fn tier_list_ranked_picks_position_stats_and_sorts_by_rank() {
        let list = parse_tier(
            r#"{
                "meta": { "version": "15.21" },
                "data": [
                    { "id": 103, "name": "Ahri", "positions": [
                        { "name": "MID", "stats": { "win_rate": 0.52, "pick_rate": 0.08, "ban_rate": 0.05, "kda": 2.8, "tier_data": { "tier": 2, "rank": 2 } },
                          "counters": [{ "champion_id": 238 }, { "champion_id": 7 }] }
                    ]},
                    { "id": 1, "name": "Annie", "positions": [
                        { "name": "MID", "stats": { "winRate": 0.55, "pickRate": 0.03, "banRate": 0.01, "kda": 3.1, "tier_data": { "tier": 1, "rank": 1 } },
                          "counters": [{ "champion_id": 55 }] }
                    ]}
                ]
            }"#,
            OpggMode::Ranked,
            Some("MID"),
        );
        assert_eq!(list.version, "15.21");
        assert_eq!(list.entries.len(), 2);
        // rank 升序：Annie(1) 在前
        assert_eq!(list.entries[0].champion_id, 1);
        assert_eq!(list.entries[0].stats.tier.as_deref(), Some("1"));
        assert_eq!(list.entries[0].stats.win_rate, Some(0.55));
        assert_eq!(list.entries[0].counters, vec![55]);
        assert_eq!(list.entries[1].champion_id, 103);
        assert_eq!(list.entries[1].counters, vec![238, 7]);
    }

    #[test]
    fn tier_list_ranked_skips_champions_without_target_position() {
        let list = parse_tier(
            r#"{
                "data": [
                    { "id": 1, "name": "Annie", "positions": [{ "name": "TOP", "stats": { "rank": 3 } }] },
                    { "id": 2, "name": "Olaf", "positions": [{ "name": "MID", "stats": { "rank": 5 } }] }
                ]
            }"#,
            OpggMode::Ranked,
            Some("MID"),
        );
        assert_eq!(list.entries.len(), 1);
        assert_eq!(list.entries[0].champion_id, 2);
        assert_eq!(list.entries[0].stats.rank, Some(5));
    }

    #[test]
    fn tier_list_non_ranked_uses_average_stats_and_requires_rank() {
        let list = parse_tier(
            r#"{
                "data": [
                    { "id": 1, "name": "Annie", "average_stats": { "winRate": 0.51, "rank": 4, "tier": "3" } },
                    { "id": 2, "name": "Olaf", "average_stats": { "winRate": 0.49 } },
                    { "id": 3, "name": "Zed" }
                ]
            }"#,
            OpggMode::Aram,
            None,
        );
        assert_eq!(list.entries.len(), 1);
        assert_eq!(list.entries[0].champion_id, 1);
        assert_eq!(list.entries[0].stats.tier.as_deref(), Some("3"));
        assert_eq!(list.entries[0].stats.win_rate, Some(0.51));
    }

    #[test]
    fn empty_payload_yields_empty_list() {
        let list = parse_tier(r#"{"data": []}"#, OpggMode::Ranked, None);
        assert!(list.entries.is_empty());
        assert!(list.version.is_empty());
    }

    #[test]
    fn build_normalizes_rates_and_zero_play_to_none() {
        let raw: RawBuildPayload = serde_json::from_str(
            r#"{
                "meta": { "version": "15.21" },
                "data": {
                    "summary": {
                        "id": 103,
                        "name": "Ahri",
                        "positions": [{ "name": "MID", "stats": { "win_rate": 0.52, "tier_data": { "tier": 1, "rank": 1 } } }],
                        "average_stats": { "win_rate": 0.5, "tier": 2 }
                    },
                    "counters": [
                        { "champion_id": 238, "name": "Zed", "play": 100, "win": 58 },
                        { "champion_id": 7, "name": "LeBlanc", "play": 0, "win": 0 }
                    ],
                    "runes": [{ "primary_page_id": 8200, "secondary_page_id": 8300,
                        "primary_rune_ids": [8214], "secondary_rune_ids": [8347], "stat_mod_ids": [5008],
                        "play": 200, "win": 110, "pick_rate": 0.4 }],
                    "skills": [{ "order": ["Q", "W", "E"], "play": 0, "win": 0 }],
                    "skill_masteries": [{ "ids": ["Q", "E", "W"] }],
                    "summoner_spells": [{ "ids": [4, 14], "play": 150, "win": 80, "pick_rate": 0.6 }],
                    "core_items": [{ "ids": [6655], "play": 90, "win": 50 }],
                    "last_items": [{ "ids": [3089], "play": 40, "win": 18 }]
                }
            }"#,
        )
        .expect("详情样本应可解析");
        let build = raw
            .data
            .map(|data| data.normalize("15.21".to_string()))
            .expect("data 存在");

        let summary = build.summary.as_ref().expect("摘要存在");
        let mid = summary.stats_for(Some("MID")).expect("MID 统计存在");
        assert_eq!(mid.tier.as_deref(), Some("1"));
        assert_eq!(
            summary.stats_for(Some("TOP")).map(|s| s.tier.clone()),
            Some(Some("2".to_string()))
        );

        let strong = build.strong_counters();
        assert_eq!(strong.len(), 1);
        assert_eq!(strong[0].champion_id, 238);
        // play=0 的条目胜率缺失，两条分组都不收录
        assert!(build.weak_counters().is_empty());

        assert_eq!(build.runes[0].win_rate, Some(0.55));
        assert_eq!(build.skills[0].win_rate, None);
        assert_eq!(build.skill_masteries[0].ids, vec!["Q", "E", "W"]);
        assert_eq!(build.summoner_spells[0].win_rate, Some(80.0 / 150.0));
        assert!(build.has_item_data());
        assert!(!build.starter_items.iter().any(|i| i.play > 0));
    }

    #[test]
    fn build_without_data_field_returns_none() {
        let raw: RawBuildPayload =
            serde_json::from_str(r#"{"meta": {"version": "15.21"}}"#).expect("样本应可解析");
        assert!(raw.data.is_none());
    }

    #[test]
    fn tier_value_accepts_number_and_string() {
        assert_eq!(TierValue::Number(1.0).display(), "1");
        assert_eq!(TierValue::Text(" 2 ".to_string()).display(), "2");
    }

    #[test]
    fn tier_list_url_omits_position_segment() {
        assert_eq!(
            tier_list_url("kr", OpggMode::Ranked),
            "https://lol-api-champion.op.gg/api/kr/champions/ranked"
        );
    }

    #[test]
    fn champion_url_matches_vue_routing() {
        assert_eq!(
            champion_url("kr", OpggMode::Ranked, 103, Some("MID")),
            "https://lol-api-champion.op.gg/api/kr/champions/ranked/103/MID"
        );
        // 非排位模式沿用 none 段
        assert_eq!(
            champion_url("global", OpggMode::Aram, 103, None),
            "https://lol-api-champion.op.gg/api/global/champions/aram/103/none"
        );
        // 斗魂竞技场走独立路由
        assert_eq!(
            champion_url("kr", OpggMode::Arena, 103, Some("MID")),
            "https://lol-api-champion.op.gg/api/kr/champions/103"
        );
    }

    #[test]
    fn item_set_blocks_dedupe_and_respect_limits() {
        let build = ChampionBuild {
            starter_items: vec![
                IdWinRate {
                    ids: vec![1055, 2003],
                    ..Default::default()
                },
                IdWinRate {
                    ids: vec![1055, 3070],
                    ..Default::default()
                },
                IdWinRate {
                    ids: vec![9999],
                    ..Default::default()
                },
            ],
            core_items: vec![
                IdWinRate {
                    ids: vec![6655],
                    ..Default::default()
                },
                IdWinRate {
                    ids: vec![6655, 3006],
                    ..Default::default()
                },
            ],
            boots: vec![IdWinRate {
                ids: vec![3020, 3006],
                ..Default::default()
            }],
            last_items: vec![
                IdWinRate {
                    ids: vec![3089, 6655],
                    ..Default::default()
                },
                IdWinRate {
                    ids: vec![3089, 3135],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let blocks = build_item_set_blocks(&build);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].block_type, "出门装备");
        // 前两组出门装去重，第三组被截断
        let starter: Vec<&str> = blocks[0].items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(starter, vec!["1055", "2003", "3070"]);
        assert_eq!(blocks[1].block_type, "核心装备");
        let core: Vec<&str> = blocks[1].items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(core, vec!["6655", "3006", "3020"]);
        assert_eq!(blocks[2].block_type, "可选与推荐装备");
        // 3089 首次出现保留，重复项与已入核心装项被剔除
        let last: Vec<&str> = blocks[2].items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(last, vec!["3089", "3135"]);
    }

    #[test]
    fn item_set_blocks_empty_when_no_item_data() {
        let blocks = build_item_set_blocks(&ChampionBuild::default());
        assert!(blocks.is_empty());
    }

    #[test]
    fn rune_perk_ids_concatenates_three_groups_in_order() {
        let preset = RunePreset {
            primary_rune_ids: vec![8214, 8226],
            secondary_rune_ids: vec![8347],
            stat_mod_ids: vec![5008, 5008, 5002],
            ..Default::default()
        };
        assert_eq!(rune_perk_ids(&preset), vec![8214, 8226, 8347, 5008, 5008, 5002]);
    }
}
