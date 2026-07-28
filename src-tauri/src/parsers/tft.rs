use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use tokio::sync::RwLock;

use crate::{build_auth_header, AppState};

static CACHED_TFT_DATA: RwLock<Option<TftDataMapping>> = RwLock::const_new(None);
static CACHED_AUGMENTS: RwLock<Option<Vec<TftAugmentInfo>>> = RwLock::const_new(None);

// ─── TFT 数据结构 ───

/// TFT 海克斯强化信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftAugmentInfo {
    pub api_name: String,
    pub name: String,
    pub desc: String,
    pub icon_path: String,
    pub tier: i32, // 1-银，2-金，3-彩/棱彩
}

/// TFT 解析后的资源映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TftDataMapping {
    pub champions: HashMap<String, String>,
    pub traits: HashMap<String, String>,
    pub champion_icons: HashMap<String, String>,
    pub trait_icons: HashMap<String, String>,
    pub item_icons: HashMap<String, String>,
    pub item_names: HashMap<String, String>,
}

/// TFT 段位信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftRankDisplay {
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

/// TFT 单个出战棋子数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftUnitDisplay {
    pub character_id: String,
    pub name: String,
    pub icon_url: String,
    pub rarity: i32,
    pub tier: i32, // 1-3星
    pub item_names: Vec<String>,
    pub item_icon_urls: Vec<String>,
}

/// TFT 单个激活羁绊数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftTraitDisplay {
    pub name: String,
    pub num_units: i32,
    pub tier_current: i32,
    pub icon_url: String,
}

/// TFT 单局中单个玩家的数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftParticipantDisplay {
    pub puuid: String,
    pub summoner_name: String,
    pub is_self: bool,
    pub placement: i32,
    pub level: i32,
    pub gold_left: i32,
    pub total_damage_to_players: i32,
    pub companion_icon_url: String,
    pub traits: Vec<TftTraitDisplay>,
    pub units: Vec<TftUnitDisplay>,
    pub augments: Vec<String>,
}

/// TFT 清洗后的单局战绩
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftMatchDisplay {
    pub game_id: u64,
    pub queue_id: i32,
    pub queue_name: String,
    pub game_creation: u64,
    pub game_duration: u64,
    pub time_str: String,
    pub duration_str: String,
    pub placement: i32, // 1-8 名
    pub level: i32,
    pub gold_left: i32,
    pub total_damage_to_players: i32,
    pub companion_icon_url: String,
    pub traits: Vec<TftTraitDisplay>,
    pub units: Vec<TftUnitDisplay>,
    pub augments: Vec<String>,
    pub participants: Vec<TftParticipantDisplay>,
}

/// TFT 战绩汇总与统计
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftMatchSummary {
    pub total_games: usize,
    pub win_count: usize,   // 登顶次数 (#1)
    pub top4_count: usize,  // 前四次数 (1-4)
    pub top4_rate: f64,     // 前四率 %
    pub win_rate: f64,      // 登顶率 %
    pub avg_placement: f64, // 平均名次
    pub matches: Vec<TftMatchDisplay>,
}

// ─── 辅助函数 ───

/// 参照 Seraphine 的 LCU 路径转换：把 .tex 换成 .png，低写处理并拼接 /lol-game-data/assets/
fn convert_seraphine_lcu_icon_path(raw_icon: &str) -> String {
    if raw_icon.is_empty() {
        return String::new();
    }
    let mut path = raw_icon.to_lowercase().replace(".tex", ".png");

    if path.starts_with("/lol-game-data/") {
        // 已经包含 /lol-game-data/ 前缀
    } else if path.starts_with('/') {
        path = format!("/lol-game-data/assets{}", path);
    } else {
        path = format!("/lol-game-data/assets/{}", path);
    }

    path.replace("//", "/")
}

fn parse_single_tft_participant(
    p: &serde_json::Value,
    pid_identity: Option<&(String, String)>,
    tft_data: &TftDataMapping,
    current_puuid: &str,
) -> TftParticipantDisplay {
    let stats = p.get("stats").unwrap_or(p);

    let (mut p_puuid, mut summoner_name) = match pid_identity {
        Some((u, n)) => (u.clone(), n.clone()),
        None => ("".to_string(), "".to_string()),
    };

    if p_puuid.is_empty() {
        p_puuid = p
            .get("puuid")
            .or_else(|| stats.get("puuid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
    }

    if summoner_name.is_empty() {
        // TFT 新版数据：riotIdGameName + riotIdTagline
        let riot_game_name = p
            .get("riotIdGameName")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let riot_tagline = p
            .get("riotIdTagline")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !riot_game_name.is_empty() {
            summoner_name = if riot_tagline.is_empty() {
                riot_game_name.to_string()
            } else {
                format!("{}#{}", riot_game_name, riot_tagline)
            };
        }
    }

    if summoner_name.is_empty() {
        summoner_name = p
            .get("summonerName")
            .or_else(|| p.get("gameName"))
            .or_else(|| stats.get("summonerName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
    }

    if summoner_name.is_empty() {
        let pid = p.get("participantId").and_then(|v| v.as_i64()).unwrap_or(0);
        if pid > 0 {
            summoner_name = format!("玩家 {}", pid);
        } else {
            summoner_name = "召唤师".to_string();
        }
    }

    let is_self = p_puuid == current_puuid;

    let placement = stats
        .get("placement")
        .or_else(|| stats.get("rank"))
        .or_else(|| p.get("placement"))
        .or_else(|| p.get("rank"))
        .and_then(|v| v.as_i64())
        .unwrap_or(8) as i32;

    let level = stats
        .get("level")
        .or_else(|| stats.get("champLevel"))
        .and_then(|v| v.as_i64())
        .unwrap_or(1) as i32;

    let gold_left = stats
        .get("goldLeft")
        .or_else(|| stats.get("gold_left"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let total_damage_to_players = stats
        .get("totalDamageToPlayers")
        .or_else(|| stats.get("total_damage_to_players"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let mut companion_icon_url = "".to_string();
    if let Some(companion) = stats.get("companion").or_else(|| p.get("companion")) {
        if let Some(item_id) = companion
            .get("skin_ID")
            .or_else(|| companion.get("item_ID"))
            .or_else(|| companion.get("skinId"))
            .and_then(|v| v.as_i64())
        {
            companion_icon_url = format!("/lol-game-data/assets/v1/profile-icons/{}.jpg", item_id);
        }
    }

    let mut units = Vec::new();
    if let Some(raw_units) = stats
        .get("units")
        .or_else(|| p.get("units"))
        .and_then(|v| v.as_array())
    {
        for u in raw_units {
            let character_id = u
                .get("character_id")
                .or_else(|| u.get("characterId"))
                .or_else(|| u.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let char_lower = character_id.to_lowercase();

            let name = tft_data
                .champions
                .get(&character_id)
                .or_else(|| tft_data.champions.get(&char_lower))
                .cloned()
                .unwrap_or_else(|| {
                    u.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&character_id)
                        .to_string()
                });

            let rarity = u.get("rarity").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let tier = u.get("tier").and_then(|v| v.as_i64()).unwrap_or(1) as i32;

            let mut item_names = Vec::new();
            let mut item_icon_urls = Vec::new();

            if let Some(arr) = u
                .get("itemNames")
                .or_else(|| u.get("items"))
                .and_then(|v| v.as_array())
            {
                for i in arr {
                    let key = if let Some(s) = i.as_str() {
                        s.to_string()
                    } else if let Some(n) = i.as_i64() {
                        n.to_string()
                    } else {
                        continue;
                    };

                    let key_lower = key.to_lowercase();

                    let translated = tft_data
                        .item_names
                        .get(&key)
                        .or_else(|| tft_data.item_names.get(&key_lower))
                        .cloned()
                        .unwrap_or_else(|| key.clone());

                    let raw_item_icon = tft_data
                        .item_icons
                        .get(&key)
                        .or_else(|| tft_data.item_icons.get(&key_lower))
                        .cloned()
                        .unwrap_or_default();

                    let item_icon_url = convert_seraphine_lcu_icon_path(&raw_item_icon);

                    item_names.push(translated);
                    if !item_icon_url.is_empty() {
                        item_icon_urls.push(item_icon_url);
                    }
                }
            }

            let raw_icon = tft_data
                .champion_icons
                .get(&character_id)
                .or_else(|| tft_data.champion_icons.get(&char_lower))
                .cloned()
                .unwrap_or_else(|| {
                    u.get("icon")
                        .or_else(|| u.get("squareIcon"))
                        .or_else(|| u.get("iconUrl"))
                        .or_else(|| u.get("iconPath"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                });

            let icon_url = convert_seraphine_lcu_icon_path(&raw_icon);

            units.push(TftUnitDisplay {
                character_id,
                name,
                icon_url,
                rarity,
                tier,
                item_names,
                item_icon_urls,
            });
        }
    }

    let mut traits = Vec::new();
    if let Some(raw_traits) = stats
        .get("traits")
        .or_else(|| p.get("traits"))
        .and_then(|v| v.as_array())
    {
        for t in raw_traits {
            let trait_api_name = t
                .get("name")
                .or_else(|| t.get("trait_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let trait_lower = trait_api_name.to_lowercase();

            let name = tft_data
                .traits
                .get(&trait_api_name)
                .or_else(|| tft_data.traits.get(&trait_lower))
                .cloned()
                .unwrap_or_else(|| trait_api_name.clone());

            let num_units = t
                .get("num_units")
                .or_else(|| t.get("numUnits"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let tier_current = t
                .get("tier_current")
                .or_else(|| t.get("tierCurrent"))
                .or_else(|| t.get("style"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            if tier_current > 0 || num_units > 0 {
                let raw_trait_icon = tft_data
                    .trait_icons
                    .get(&trait_api_name)
                    .or_else(|| tft_data.trait_icons.get(&trait_lower))
                    .cloned()
                    .unwrap_or_else(|| {
                        t.get("icon")
                            .or_else(|| t.get("iconPath"))
                            .or_else(|| t.get("iconUrl"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string()
                    });

                let icon_url = convert_seraphine_lcu_icon_path(&raw_trait_icon);

                traits.push(TftTraitDisplay {
                    name,
                    num_units,
                    tier_current,
                    icon_url,
                });
            }
        }
    }

    let mut augments = Vec::new();
    if let Some(raw_augments) = stats
        .get("augments")
        .or_else(|| p.get("augments"))
        .and_then(|v| v.as_array())
    {
        augments = raw_augments
            .iter()
            .filter_map(|a| {
                if let Some(s) = a.as_str() {
                    Some(s.to_string())
                } else {
                    a.as_i64().map(|n| n.to_string())
                }
            })
            .collect();
    }

    TftParticipantDisplay {
        puuid: p_puuid,
        summoner_name,
        is_self,
        placement,
        level,
        gold_left,
        total_damage_to_players,
        companion_icon_url,
        traits,
        units,
        augments,
    }
}

// ─── 解析逻辑 ───

/// 从 TFT JSON 内容解析资源映射
fn parse_tft_data_from_value(root: &serde_json::Value) -> TftDataMapping {
    let mut mapping = TftDataMapping {
        champions: HashMap::new(),
        traits: HashMap::new(),
        champion_icons: HashMap::new(),
        trait_icons: HashMap::new(),
        item_icons: HashMap::new(),
        item_names: HashMap::new(),
    };

    let mut process_set_obj = |set_obj: &serde_json::Value| {
        if let Some(champs) = set_obj.get("champions").and_then(|v| v.as_array()) {
            for c in champs {
                if let Some(api_name) = c.get("apiName").and_then(|v| v.as_str()) {
                    let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let icon = c
                        .get("squareIcon")
                        .or_else(|| c.get("icon"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let api_lower = api_name.to_lowercase();
                    mapping
                        .champions
                        .insert(api_name.to_string(), name.to_string());
                    mapping
                        .champions
                        .insert(api_lower.clone(), name.to_string());

                    if !icon.is_empty() {
                        let icon_converted = convert_seraphine_lcu_icon_path(icon);
                        mapping
                            .champion_icons
                            .insert(api_name.to_string(), icon_converted.clone());
                        mapping.champion_icons.insert(api_lower, icon_converted);
                    }
                }
            }
        }
        if let Some(traits) = set_obj.get("traits").and_then(|v| v.as_array()) {
            for t in traits {
                if let Some(api_name) = t.get("apiName").and_then(|v| v.as_str()) {
                    let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let icon = t.get("icon").and_then(|v| v.as_str()).unwrap_or("");
                    let api_lower = api_name.to_lowercase();
                    mapping
                        .traits
                        .insert(api_name.to_string(), name.to_string());
                    mapping.traits.insert(api_lower.clone(), name.to_string());

                    if !icon.is_empty() {
                        let icon_converted = convert_seraphine_lcu_icon_path(icon);
                        mapping
                            .trait_icons
                            .insert(api_name.to_string(), icon_converted.clone());
                        mapping.trait_icons.insert(api_lower, icon_converted);
                    }
                }
            }
        }
    };

    if let Some(set_data) = root.get("setData").and_then(|v| v.as_array()) {
        for set_obj in set_data {
            process_set_obj(set_obj);
        }
    }

    if let Some(sets) = root.get("sets").and_then(|v| v.as_object()) {
        for (_k, set_obj) in sets {
            process_set_obj(set_obj);
        }
    }

    let mut process_item = |item: &serde_json::Value| {
        if let Some(api_name) = item.get("apiName").and_then(|v| v.as_str()) {
            let api_lower = api_name.to_lowercase();
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                mapping
                    .item_names
                    .insert(api_name.to_string(), name.to_string());
                mapping
                    .item_names
                    .insert(api_lower.clone(), name.to_string());
            }
            if let Some(icon) = item.get("icon").and_then(|v| v.as_str()) {
                let icon_converted = convert_seraphine_lcu_icon_path(icon);
                mapping
                    .item_icons
                    .insert(api_name.to_string(), icon_converted.clone());
                mapping.item_icons.insert(api_lower, icon_converted);
            }
        }
    };

    if let Some(items) = root.get("items").and_then(|v| v.as_array()) {
        for item in items {
            process_item(item);
        }
    }

    if let Some(augments) = root.get("augments").and_then(|v| v.as_array()) {
        for aug in augments {
            process_item(aug);
        }
    }

    mapping
}

fn get_tft_data_cache_path() -> Option<std::path::PathBuf> {
    let dir = dirs::config_dir()?.join("Yuumi").join("cache");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("tft_data.json"))
}

fn get_tft_augments_cache_path() -> Option<std::path::PathBuf> {
    let dir = dirs::config_dir()?.join("Yuumi").join("cache");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("tft_augments.json"))
}

fn clean_tft_desc(desc_raw: &str) -> String {
    if desc_raw.is_empty() {
        return String::new();
    }
    let mut result = String::with_capacity(desc_raw.len());
    let mut in_tag = false;
    for ch in desc_raw.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n")
        .trim()
        .to_string()
}

fn extract_augment_tier(api_name: &str, icon: &str, name: &str) -> i32 {
    let lower_api = api_name.to_lowercase();
    let lower_icon = icon.to_lowercase();

    if lower_api.contains("prismatic")
        || lower_icon.contains("prismatic")
        || lower_api.contains("tier3")
        || lower_icon.contains("tier3")
        || lower_api.contains("hr_t3")
    {
        return 3;
    }
    if lower_api.contains("gold")
        || lower_icon.contains("gold")
        || lower_api.contains("tier2")
        || lower_icon.contains("tier2")
        || lower_api.contains("hr_t2")
    {
        return 2;
    }
    if lower_api.contains("silver")
        || lower_icon.contains("silver")
        || lower_api.contains("tier1")
        || lower_icon.contains("tier1")
        || lower_api.contains("hr_t1")
    {
        return 1;
    }

    if name.ends_with(" III") || name.contains(" III ") {
        return 3;
    }
    if name.ends_with(" II") || name.contains(" II ") {
        return 2;
    }
    if name.ends_with(" I") || name.contains(" I ") {
        return 1;
    }

    2
}

fn extract_augments_from_value(root: &serde_json::Value) -> Vec<TftAugmentInfo> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut process_item_val = |item: &serde_json::Value, force_augment: bool| {
        let name = item
            .get("name")
            .or_else(|| item.get("displayName"))
            .or_else(|| item.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.is_empty() {
            return;
        }

        let api_name = item
            .get("apiName")
            .or_else(|| item.get("api_name"))
            .or_else(|| item.get("hexId"))
            .and_then(|v| v.as_str())
            .unwrap_or(name);
        if seen.contains(api_name) {
            return;
        }

        let icon = item
            .get("icon")
            .or_else(|| item.get("squareIcon"))
            .or_else(|| item.get("imgUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let lower_api = api_name.to_lowercase();
        let lower_icon = icon.to_lowercase();

        let is_aug = force_augment
            || lower_api.contains("augment")
            || lower_icon.contains("augment")
            || (lower_api.starts_with("tft")
                && (lower_api.contains("aug")
                    || lower_api.contains("superrune")
                    || lower_api.contains("hextech")));

        if !is_aug {
            return;
        }

        seen.insert(api_name.to_string());

        let raw_desc = item
            .get("desc")
            .or_else(|| item.get("description"))
            .or_else(|| item.get("descClean"))
            .or_else(|| item.get("tooltip"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let desc = clean_tft_desc(raw_desc);
        let icon_path = convert_seraphine_lcu_icon_path(icon);
        let tier = extract_augment_tier(api_name, icon, name);

        result.push(TftAugmentInfo {
            api_name: api_name.to_string(),
            name: name.to_string(),
            desc,
            icon_path,
            tier,
        });
    };

    let root_keys: Vec<&str> = root
        .as_object()
        .map(|m| m.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();

    // 1. 根节点自身为 Array（某些 LCU 端点的裸数组结构）
    if let Some(arr) = root.as_array() {
        log::info!("[Extract] 根节点为 Array，共 {} 个元素", arr.len());
        for item in arr {
            process_item_val(item, false);
        }
    }

    // 2. 根节点为 Object — 从各字段提取
    let has_augments_field = root_keys.contains(&"augments");
    let has_set_data = root_keys.contains(&"setData");
    let has_sets = root_keys.contains(&"sets");
    let has_items = root_keys.contains(&"items");
    log::info!(
        "[Extract] Object 字段: augments={}, setData={}, sets={}, items={}",
        has_augments_field,
        has_set_data,
        has_sets,
        has_items
    );

    if let Some(arr) = root.get("augments").and_then(|v| v.as_array()) {
        log::info!("[Extract] 根节点 augments[] = {} 项", arr.len());
        for item in arr {
            process_item_val(item, true);
        }
    }

    if let Some(arr) = root.get("items").and_then(|v| v.as_array()) {
        for item in arr {
            process_item_val(item, false);
        }
    }

    if let Some(set_data) = root.get("setData").and_then(|v| v.as_array()) {
        for set_obj in set_data {
            if let Some(items) = set_obj.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    process_item_val(item, false);
                }
            }
            if let Some(augments) = set_obj.get("augments").and_then(|v| v.as_array()) {
                for item in augments {
                    process_item_val(item, true);
                }
            }
        }
    }

    if let Some(sets) = root.get("sets").and_then(|v| v.as_object()) {
        for (_k, set_obj) in sets {
            if let Some(items) = set_obj.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    process_item_val(item, false);
                }
            }
            if let Some(augments) = set_obj.get("augments").and_then(|v| v.as_array()) {
                for item in augments {
                    process_item_val(item, true);
                }
            }
        }
    }

    result
}

/// 抓取 TFT 基础数据字典（优先 LCU，备用 CDragon，带内存与磁盘缓存）
/// 参照 Seraphine：LCU → CDragon（一次，无重试，无代理）
async fn fetch_tft_data_mapping(_lcu: Option<&crate::LcuClient>) -> TftDataMapping {
    let mut cache = CACHED_TFT_DATA.write().await;
    if let Some(ref mapping) = *cache {
        if !mapping.champions.is_empty() {
            return mapping.clone();
        }
    }

    // 1. 尝试从本地磁盘缓存加载
    if let Some(cache_path) = get_tft_data_cache_path() {
        if cache_path.exists() {
            if let Ok(file_content) = std::fs::read_to_string(&cache_path) {
                if let Ok(m) = serde_json::from_str::<TftDataMapping>(&file_content) {
                    if !m.champions.is_empty() {
                        *cache = Some(m.clone());
                        return m;
                    }
                }
            }
        }
    }

    // 2. 参照 Seraphine update 方法：CDragon CDN 一次请求不重试
    let cdn_url = "https://raw.communitydragon.org/latest/cdragon/tft/zh_cn.json";
    let cdn_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    if let Ok(resp) = cdn_client.get(cdn_url).send().await {
        if resp.status().is_success() {
            if let Ok(root_val) = resp.json::<serde_json::Value>().await {
                let m = parse_tft_data_from_value(&root_val);
                if !m.champions.is_empty() {
                    *cache = Some(m.clone());
                    let m_clone = m.clone();
                    let aug_list = extract_augments_from_value(&root_val);
                    log::info!("从 CDragon 成功提取到 {} 个 TFT 海克斯强化", aug_list.len());
                    if !aug_list.is_empty() {
                        let mut aug_cache = CACHED_AUGMENTS.write().await;
                        *aug_cache = Some(aug_list.clone());
                        tokio::spawn(async move {
                            if let Some(aug_path) = get_tft_augments_cache_path() {
                                if let Ok(json_str) = serde_json::to_string(&aug_list) {
                                    let _ = std::fs::write(aug_path, json_str);
                                }
                            }
                        });
                    }
                    tokio::spawn(async move {
                        if let Some(cache_path) = get_tft_data_cache_path() {
                            if let Ok(json_str) = serde_json::to_string(&m_clone) {
                                let _ = std::fs::write(cache_path, json_str);
                            }
                        }
                    });
                    return m;
                }
            }
        }
    }

    TftDataMapping {
        champions: HashMap::new(),
        traits: HashMap::new(),
        champion_icons: HashMap::new(),
        trait_icons: HashMap::new(),
        item_icons: HashMap::new(),
        item_names: HashMap::new(),
    }
}

async fn fetch_tft_augments_raw(app_state: &AppState) -> Vec<TftAugmentInfo> {
    log::info!("[TFT Augments Backend] 准备抓取海克斯强化 (CDragon CDN)");

    // 确切有效的 CDN 路径
    let cdn_urls = ["https://raw.communitydragon.org/latest/cdragon/tft/zh_cn.json"];

    let (enable_proxy, proxy_addr) = {
        let cfg = app_state.config.read().await;
        (
            cfg.general.enable_opgg_proxy,
            cfg.general.opgg_proxy_addr.trim().to_string(),
        )
    };

    let mut builder = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(60));

    if enable_proxy && !proxy_addr.is_empty() {
        let formatted_proxy = if !proxy_addr.starts_with("http://")
            && !proxy_addr.starts_with("https://")
            && !proxy_addr.starts_with("socks5://")
        {
            format!("http://{}", proxy_addr)
        } else {
            proxy_addr.clone()
        };

        if let Ok(p) = reqwest::Proxy::all(&formatted_proxy) {
            log::info!("[TFT CDN] CDragon 请求已成功配置代理: {}", formatted_proxy);
            builder = builder.proxy(p);
        } else {
            log::warn!("[TFT CDN] 代理地址解析失败: {}", formatted_proxy);
        }
    }

    let cdn_client = builder.build().unwrap_or_else(|_| reqwest::Client::new());

    for cdn_url in cdn_urls {
        log::info!("[TFT CDN] 尝试: {}", cdn_url);

        match cdn_client.get(cdn_url).send().await {
            Ok(resp) => {
                let status = resp.status();
                log::info!("[TFT CDN] HTTP {}", status);

                if status.is_success() {
                    match resp.bytes().await {
                        Ok(bytes) => {
                            log::info!("[TFT CDN] 下载 {} bytes", bytes.len());
                            match serde_json::from_slice::<serde_json::Value>(&bytes) {
                                Ok(root_val) => {
                                    let aug_list = extract_augments_from_value(&root_val);
                                    log::info!("[TFT CDN] 提取到 {} 个海克斯强化", aug_list.len());
                                    if !aug_list.is_empty() {
                                        return aug_list;
                                    }
                                }
                                Err(e) => {
                                    log::warn!("[TFT CDN] JSON 解析失败: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("[TFT CDN] Body 读取失败: {:?}", e);
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("[TFT CDN] 请求失败: {:?}", e);
            }
        }
    }

    Vec::new()
}

// ─── Tauri 命令 ───

/// 从 LCU 获取 TFT 数据资源
#[tauri::command]
pub async fn get_tft_data(app_state: State<'_, AppState>) -> Result<TftDataMapping, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    Ok(fetch_tft_data_mapping(Some(lcu)).await)
}

/// 从已解析的 tft.json 缓存中提取所有海克斯强化信息（支持强制跳过缓存刷新）
#[tauri::command]
pub async fn get_tft_augments(
    force_refresh: Option<bool>,
    app_state: State<'_, AppState>,
) -> Result<Vec<TftAugmentInfo>, String> {
    let force = force_refresh.unwrap_or(false);
    log::info!(
        "[Tauri Cmd] >>> get_tft_augments 命令被触发！(force_refresh={})",
        force
    );

    if !force {
        // 1. 尝试读取内存缓存
        {
            let cache = CACHED_AUGMENTS.read().await;
            if let Some(ref list) = *cache {
                if !list.is_empty() {
                    return Ok(list.clone());
                }
            }
        }

        // 2. 尝试读取本地磁盘缓存
        if let Some(path) = get_tft_augments_cache_path() {
            if path.exists() {
                if let Ok(file_content) = std::fs::read_to_string(&path) {
                    if let Ok(list) = serde_json::from_str::<Vec<TftAugmentInfo>>(&file_content) {
                        if !list.is_empty() {
                            let mut cache = CACHED_AUGMENTS.write().await;
                            *cache = Some(list.clone());
                            return Ok(list);
                        } else {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }

    // 3. CDragon CDN 抓取（支持系统/用户代理）
    let list = fetch_tft_augments_raw(&app_state).await;

    if !list.is_empty() {
        let mut cache = CACHED_AUGMENTS.write().await;
        *cache = Some(list.clone());
        let list_for_cache = list.clone();
        tokio::spawn(async move {
            if let Some(aug_path) = get_tft_augments_cache_path() {
                if let Ok(json_str) = serde_json::to_string(&list_for_cache) {
                    let _ = std::fs::write(aug_path, json_str);
                }
            }
        });
        return Ok(list);
    }

    // 4. 若网络重新抓取失败或超时，安全回滚：优先使用现有的本地磁盘或内存缓存，避免清空界面数据
    log::warn!("TFT 海克斯网络抓取失败或超时，尝试安全回滚使用本地缓存");
    {
        let cache = CACHED_AUGMENTS.read().await;
        if let Some(ref old_list) = *cache {
            if !old_list.is_empty() {
                return Ok(old_list.clone());
            }
        }
    }
    if let Some(path) = get_tft_augments_cache_path() {
        if path.exists() {
            if let Ok(file_content) = std::fs::read_to_string(&path) {
                if let Ok(old_list) = serde_json::from_str::<Vec<TftAugmentInfo>>(&file_content) {
                    if !old_list.is_empty() {
                        return Ok(old_list);
                    }
                }
            }
        }
    }

    Ok(Vec::new())
}

/// 获取当前召唤师的云顶之弈段位数据
#[tauri::command]
pub async fn get_tft_ranked_stats(
    puuid: String,
    app_state: State<'_, AppState>,
) -> Result<TftRankDisplay, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let url = format!(
        "https://127.0.0.1:{}/lol-ranked/v1/ranked-stats/{}",
        lcu.port, puuid
    );
    let auth = build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("请求云顶段位失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取云顶段位失败: HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let mut rank_display = TftRankDisplay {
        solo_tier: "UNRANKED".to_string(),
        solo_division: "NA".to_string(),
        solo_lp: 0,
        solo_wins: 0,
        solo_losses: 0,
        turbo_tier: "NONE".to_string(),
        turbo_rating: 0,
        turbo_wins: 0,
        double_tier: "UNRANKED".to_string(),
        double_division: "NA".to_string(),
        double_lp: 0,
        double_wins: 0,
        double_losses: 0,
    };

    if let Some(queues) = json.get("queues").and_then(|q| q.as_array()) {
        for q in queues {
            let queue_type = q.get("queueType").and_then(|v| v.as_str()).unwrap_or("");
            match queue_type {
                "RANKED_TFT" => {
                    rank_display.solo_tier = q
                        .get("tier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNRANKED")
                        .to_string();
                    rank_display.solo_division = q
                        .get("division")
                        .or_else(|| q.get("rank"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("NA")
                        .to_string();
                    rank_display.solo_lp =
                        q.get("leaguePoints").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.solo_wins =
                        q.get("wins").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.solo_losses =
                        q.get("losses").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                }
                "RANKED_TFT_TURBO" => {
                    rank_display.turbo_tier = q
                        .get("ratedTier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("NONE")
                        .to_string();
                    rank_display.turbo_rating =
                        q.get("ratedRating").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.turbo_wins =
                        q.get("wins").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                }
                "RANKED_TFT_DOUBLE_UP" => {
                    rank_display.double_tier = q
                        .get("tier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNRANKED")
                        .to_string();
                    rank_display.double_division = q
                        .get("division")
                        .or_else(|| q.get("rank"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("NA")
                        .to_string();
                    rank_display.double_lp =
                        q.get("leaguePoints").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.double_wins =
                        q.get("wins").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.double_losses =
                        q.get("losses").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                }
                _ => {}
            }
        }
    }

    Ok(rank_display)
}

/// 获取云顶之弈战绩列表与汇总统计
#[tauri::command]
pub async fn get_tft_match_history(
    puuid: String,
    beg_index: Option<u32>,
    end_index: Option<u32>,
    app_state: State<'_, AppState>,
) -> Result<TftMatchSummary, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    // 预先拉取/建立 TFT 资源与图标映射字典
    let tft_data = fetch_tft_data_mapping(Some(lcu)).await;

    let auth = build_auth_header(&lcu.token);

    let b = beg_index.unwrap_or(0);
    let e = end_index.unwrap_or(20);
    let count = if e >= b { e - b + 1 } else { 20 };

    let candidate_urls = vec![
        format!(
            "https://127.0.0.1:{}/lol-match-history/v1/products/tft/{}/matches?begIndex={}&endIndex={}",
            lcu.port, puuid, b, e
        ),
        format!(
            "https://127.0.0.1:{}/lol-match-history/v1/products/tft/{}/matches?beginIndex={}&endIndex={}",
            lcu.port, puuid, b, e
        ),
        format!(
            "https://127.0.0.1:{}/lol-match-history/v1/products/tft/{}/matches",
            lcu.port, puuid
        ),
    ];

    let mut raw_json: Option<serde_json::Value> = None;

    for url in candidate_urls {
        if let Ok(resp) = lcu
            .http_client
            .get(&url)
            .header("Authorization", &auth)
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    raw_json = Some(json);
                    break;
                }
            }
        }
    }

    // 若 LCU 本地接口未成功获取，尝试使用 SGP 接口获取
    if raw_json.is_none() {
        if let Some(server) = &lcu.server {
            let server_lower = server.to_lowercase();
            const TENCENT_SERVERS: &[&str] = &[
                "hn1", "hn10", "bgp2", "tj100", "cq100", "gz100", "nj100", "tj101",
            ];
            if TENCENT_SERVERS.contains(&server_lower.as_str()) {
                let token_url = format!("https://127.0.0.1:{}/entitlements/v1/token", lcu.port);
                if let Ok(token_resp) = lcu
                    .http_client
                    .get(&token_url)
                    .header("Authorization", &auth)
                    .send()
                    .await
                {
                    if token_resp.status().is_success() {
                        if let Ok(token_data) = token_resp.json::<serde_json::Value>().await {
                            if let Some(sgp_token) =
                                token_data.get("accessToken").and_then(|v| v.as_str())
                            {
                                const K8S_SGP_SERVERS: &[&str] = &["hn1", "hn10", "bgp2"];
                                let sgp_base = if K8S_SGP_SERVERS.contains(&server_lower.as_str()) {
                                    format!("https://{}-k8s-sgp.lol.qq.com:21019", server_lower)
                                } else {
                                    format!("https://{}-sgp.lol.qq.com:21019", server_lower)
                                };
                                let sgp_url = format!(
                                    "{}/match-history-query/v1/products/tft/player/{}/SUMMARY",
                                    sgp_base, puuid
                                );
                                if let Ok(sgp_client) = reqwest::Client::builder()
                                    .danger_accept_invalid_certs(true)
                                    .no_proxy()
                                    .build()
                                {
                                    if let Ok(sgp_resp) = sgp_client
                                        .get(&sgp_url)
                                        .header("Authorization", format!("Bearer {}", sgp_token))
                                        .query(&[
                                            ("startIndex", &b.to_string()),
                                            ("count", &count.to_string()),
                                        ])
                                        .send()
                                        .await
                                    {
                                        if sgp_resp.status().is_success() {
                                            if let Ok(sgp_json) =
                                                sgp_resp.json::<serde_json::Value>().await
                                            {
                                                raw_json = Some(sgp_json);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let raw_json = match raw_json {
        Some(j) => j,
        None => {
            log::warn!("无法获取云顶战绩数据，返回空统计");
            return Ok(TftMatchSummary {
                total_games: 0,
                win_count: 0,
                top4_count: 0,
                top4_rate: 0.0,
                win_rate: 0.0,
                avg_placement: 0.0,
                matches: Vec::new(),
            });
        }
    };

    let games = raw_json
        .get("games")
        .and_then(|g| g.get("games").or(Some(g)))
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default();

    let mut cleaned_matches = Vec::new();
    let mut win_count = 0;
    let mut top4_count = 0;
    let mut total_placement_sum = 0;

    for raw_g in &games {
        // 解包 SGP 或 LCU 结构中的 json 嵌套
        let g_obj = if let Some(j_val) = raw_g.get("json") {
            if j_val.is_string() {
                serde_json::from_str::<serde_json::Value>(j_val.as_str().unwrap())
                    .unwrap_or_else(|_| raw_g.clone())
            } else if j_val.is_object() {
                j_val.clone()
            } else {
                raw_g.clone()
            }
        } else {
            raw_g.clone()
        };

        let game_id = g_obj.get("gameId").and_then(|v| v.as_u64()).unwrap_or(0);
        let game_creation = g_obj
            .get("gameCreation")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let raw_duration = g_obj
            .get("game_length")
            .or_else(|| g_obj.get("gameDuration"))
            .or_else(|| g_obj.get("gameLength"));

        let game_duration = raw_duration
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(0);
        let queue_id = g_obj.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let queue_name = match queue_id {
            1090 => "云顶之弈(匹配)",
            1100 => "云顶之弈(排位)",
            1130 => "狂暴模式",
            1160 => "双人作战",
            _ => "云顶模式",
        }
        .to_string();

        let secs = (game_creation / 1000) as i64;
        let time_str = chrono::DateTime::from_timestamp(secs, 0)
            .map(|dt| {
                let utc8_fixed = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
                dt.with_timezone(&utc8_fixed)
                    .format("%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "01-01 00:00".to_string());

        let duration_str = format!("{:02}:{:02}", game_duration / 60, game_duration % 60);

        let raw_participants = g_obj
            .get("participants")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // participantId → (puuid, name)，用于 SR 风格数据
        let mut pid_map: HashMap<i64, (String, String)> = HashMap::new();
        // puuid → name，用于 TFT 风格数据（participants 无 participantId）
        let mut puuid_name_map: HashMap<String, String> = HashMap::new();

        if let Some(identities) = g_obj
            .get("participantIdentities")
            .and_then(|v| v.as_array())
        {
            for item in identities {
                let pid = item
                    .get("participantId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                if let Some(player) = item.get("player") {
                    let player_puuid = player
                        .get("puuid")
                        .or_else(|| player.get("currentPuuid"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let game_name = player
                        .get("gameName")
                        .or_else(|| player.get("game_name"))
                        .and_then(|v| v.as_str());

                    let tag_line = player
                        .get("tagLine")
                        .or_else(|| player.get("tag_line"))
                        .and_then(|v| v.as_str());

                    let summoner_name = if let (Some(gn), Some(tl)) = (game_name, tag_line) {
                        format!("{}#{}", gn, tl)
                    } else if let Some(gn) = game_name {
                        gn.to_string()
                    } else if let Some(sn) = player
                        .get("summonerName")
                        .or_else(|| player.get("displayName"))
                        .or_else(|| player.get("name"))
                        .and_then(|v| v.as_str())
                    {
                        sn.to_string()
                    } else {
                        "".to_string()
                    };

                    if pid > 0 {
                        pid_map.insert(pid, (player_puuid.clone(), summoner_name.clone()));
                    }
                    if !player_puuid.is_empty() && !summoner_name.is_empty() {
                        puuid_name_map.insert(player_puuid, summoner_name);
                    }
                }
            }
        }

        let mut parsed_participants = Vec::new();
        for p_val in &raw_participants {
            let pid = p_val
                .get("participantId")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            // 优先用 participantId 查（SR 风格），取不到再用 participant 自身 puuid 查（TFT 风格）
            let pid_identity = if pid > 0 { pid_map.get(&pid) } else { None };
            let p_puuid_for_lookup = p_val
                .get("puuid")
                .or_else(|| p_val.get("stats").and_then(|s| s.get("puuid")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let puuid_identity: Option<(String, String)> =
                if pid_identity.is_none() && !p_puuid_for_lookup.is_empty() {
                    puuid_name_map
                        .get(p_puuid_for_lookup)
                        .map(|name| (p_puuid_for_lookup.to_string(), name.clone()))
                } else {
                    None
                };
            let effective_identity = pid_identity.or(puuid_identity.as_ref());
            let parsed_p =
                parse_single_tft_participant(p_val, effective_identity, &tft_data, &puuid);
            parsed_participants.push(parsed_p);
        }

        // 按名次 (#1 ~ #8) 升序排序
        parsed_participants.sort_by_key(|p| p.placement);

        let my_p = parsed_participants
            .iter()
            .find(|p| p.is_self)
            .cloned()
            .unwrap_or_else(|| {
                parsed_participants
                    .first()
                    .cloned()
                    .unwrap_or(TftParticipantDisplay {
                        puuid: puuid.clone(),
                        summoner_name: "我".to_string(),
                        is_self: true,
                        placement: 8,
                        level: 1,
                        gold_left: 0,
                        total_damage_to_players: 0,
                        companion_icon_url: "".to_string(),
                        traits: Vec::new(),
                        units: Vec::new(),
                        augments: Vec::new(),
                    })
            });

        if my_p.placement == 1 {
            win_count += 1;
        }
        if my_p.placement <= 4 {
            top4_count += 1;
        }
        total_placement_sum += my_p.placement;

        cleaned_matches.push(TftMatchDisplay {
            game_id,
            queue_id,
            queue_name,
            game_creation,
            game_duration,
            time_str,
            duration_str,
            placement: my_p.placement,
            level: my_p.level,
            gold_left: my_p.gold_left,
            total_damage_to_players: my_p.total_damage_to_players,
            companion_icon_url: my_p.companion_icon_url,
            traits: my_p.traits,
            units: my_p.units,
            augments: my_p.augments,
            participants: parsed_participants,
        });
    }

    let total_games = cleaned_matches.len();
    let (top4_rate, win_rate, avg_placement) = if total_games > 0 {
        (
            (top4_count as f64 / total_games as f64) * 100.0,
            (win_count as f64 / total_games as f64) * 100.0,
            total_placement_sum as f64 / total_games as f64,
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
        matches: cleaned_matches,
    })
}

/// 构建归一化的 trait name 映射（从 TftDataMapping 提取，不含网络请求）
fn build_trait_name_map_from_mapping(mapping: &TftDataMapping) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (api_name, display_name) in &mapping.traits {
        let parts: Vec<&str> = api_name.split('_').collect();
        let key = if parts.last().copied() == Some("Trait") && parts.len() > 1 {
            parts[parts.len() - 2]
        } else {
            parts.last().copied().unwrap_or(api_name.as_str())
        };
        let key = if key.ends_with("Trait") && key.len() > 5 {
            &key[..key.len() - 5]
        } else {
            key
        };
        let key_lower = key.to_lowercase();
        if !key_lower.is_empty() {
            result
                .entry(key_lower.clone())
                .or_insert_with(|| display_name.clone());
            result
                .entry(format!("{}trait", key_lower))
                .or_insert_with(|| display_name.clone());
        }
    }
    result
}

/// 尝试获取 TFT 数据映射填充缓存（LCU 优先，CDragon 兜底）
async fn ensure_tft_data_mapping(lcu: Option<&crate::LcuClient>) {
    fetch_tft_data_mapping(lcu).await;
}

/// 从内存缓存或 LCU/CDragon 获取 TFT 羁绊中文名称映射
pub async fn fetch_tft_meta_maps(lcu: Option<&crate::LcuClient>) -> serde_json::Value {
    ensure_tft_data_mapping(lcu).await;

    let cached = CACHED_TFT_DATA.read().await;
    let (trait_map, champ_icon_map, champ_name_map, item_name_map, item_icon_map) = match cached
        .as_ref()
    {
        Some(m) if !m.traits.is_empty() => {
            log::info!("TFT meta maps: 从缓存构建 ({} traits, {} champIcons, {} champNames, {} items, {} itemIcons)",
                m.traits.len(), m.champion_icons.len(), m.champions.len(), m.item_names.len(), m.item_icons.len());
            // 从 champion_icons 的 key 构建名称映射作为兜底
            let name_map = if m.champions.is_empty() {
                log::warn!("TFT meta maps: champion_name_map 为空，从 icon 键构建");
                let mut fallback = HashMap::new();
                for api_name in m.champion_icons.keys() {
                    let display = api_name.split('_').next_back().unwrap_or(api_name);
                    fallback.insert(api_name.clone(), display.to_string());
                    fallback.insert(api_name.to_lowercase(), display.to_string());
                }
                fallback
            } else {
                m.champions.clone()
            };
            // 将 champion_icons 路径转为 CDragon 直链（去掉 LCU 依赖）
            let cdn_icon_map: HashMap<String, String> = m
                .champion_icons
                .iter()
                .map(|(k, v)| {
                    let cdn = convert_seraphine_lcu_icon_path_to_cdragon(v);
                    (k.clone(), cdn)
                })
                .collect();
            // 将 item_icons 路径转为 CDragon 直链
            let cdn_item_icon_map: HashMap<String, String> = m
                .item_icons
                .iter()
                .map(|(k, v)| {
                    let cdn = convert_seraphine_lcu_icon_path_to_cdragon(v);
                    (k.clone(), cdn)
                })
                .collect();
            (
                build_trait_name_map_from_mapping(m),
                cdn_icon_map,
                name_map,
                m.item_names.clone(),
                cdn_item_icon_map,
            )
        }
        _ => {
            log::warn!("TFT meta maps: 无可用数据，返回空映射");
            (
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            )
        }
    };
    drop(cached);

    let mut result = serde_json::Map::new();
    result.insert(
        "trait_name_map".into(),
        serde_json::to_value(&trait_map).unwrap_or_default(),
    );
    result.insert(
        "champion_icon_map".into(),
        serde_json::to_value(&champ_icon_map).unwrap_or_default(),
    );
    result.insert(
        "champion_name_map".into(),
        serde_json::to_value(&champ_name_map).unwrap_or_default(),
    );
    result.insert(
        "item_name_map".into(),
        serde_json::to_value(&item_name_map).unwrap_or_default(),
    );
    result.insert(
        "item_icon_map".into(),
        serde_json::to_value(&item_icon_map).unwrap_or_default(),
    );
    serde_json::Value::Object(result)
}

/// 将 LCU asset 路径转为 CDragon 直链
///
/// tft.json 里常见原始路径是 `ASSETS/Characters/.../*.tex`（无 `/lol-game-data` 前缀）。
/// 必须先归一化成 LCU 路径，再映射到 CDragon 的 `.../latest/game/...`，
/// 否则会生成 `.../latest/assets/...` 这种 404 URL。
fn convert_seraphine_lcu_icon_path_to_cdragon(raw_icon: &str) -> String {
    if raw_icon.is_empty() {
        return String::new();
    }
    // 复用 LCU 路径归一化（补全 /lol-game-data/assets、.tex→.png、小写）
    let path = convert_seraphine_lcu_icon_path(raw_icon);
    if path.starts_with("/lol-game-data/assets/") {
        let sub = path.strip_prefix("/lol-game-data/assets/").unwrap_or(&path);
        format!("https://raw.communitydragon.org/latest/game/{}", sub)
    } else if path.starts_with("/lol-game-data/") {
        let sub = path.strip_prefix("/lol-game-data/").unwrap_or(&path);
        format!(
            "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/{}",
            sub
        )
    } else {
        // 兜底：仍走 game/ 前缀，避免再生成无效的 latest/<path>
        format!(
            "https://raw.communitydragon.org/latest/game/{}",
            path.trim_start_matches('/')
        )
    }
}
