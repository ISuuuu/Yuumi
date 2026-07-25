use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use tokio::sync::RwLock;

use crate::{build_auth_header, AppState};

static CACHED_TFT_DATA: RwLock<Option<TftDataMapping>> = RwLock::const_new(None);

// ─── TFT 数据结构 ───

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

// ─── LCU 原始 JSON 结构 ───

#[derive(Debug, Clone, Deserialize)]
struct TftJsonRoot {
    #[serde(rename = "setData")]
    set_data: Option<Vec<TftSet>>,
    sets: Option<HashMap<String, TftSet>>,
    items: Option<Vec<TftGenericItem>>,
    augments: Option<Vec<TftGenericItem>>,
}

#[derive(Debug, Clone, Deserialize)]
struct TftSet {
    champions: Option<Vec<TftChampion>>,
    traits: Option<Vec<TftTrait>>,
}

#[derive(Debug, Clone, Deserialize)]
struct TftChampion {
    #[serde(rename = "apiName")]
    api_name: Option<String>,
    name: Option<String>,
    #[serde(rename = "squareIcon")]
    square_icon: Option<String>,
    icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TftTrait {
    #[serde(rename = "apiName")]
    api_name: Option<String>,
    name: Option<String>,
    icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TftGenericItem {
    #[serde(rename = "apiName")]
    api_name: Option<String>,
    name: Option<String>,
    icon: Option<String>,
}

// ─── 辅助函数 ───

/// 参照 Seraphine 的 LCU 路径转换：把 .tex 换成 .png，低写处理并拼接 /lol-game-data/assets/
fn convert_seraphine_lcu_icon_path(raw_icon: &str) -> String {
    if raw_icon.is_empty() {
        return "".to_string();
    }
    let mut path = raw_icon.replace(".tex", ".png").replace(".TEX", ".png");
    path = path.to_lowercase();

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

/// 从 TFT JSON 内容解析资源映射（对应 Python `parseData`）
fn parse_tft_data(content: &TftJsonRoot) -> TftDataMapping {
    let mut mapping = TftDataMapping {
        champions: HashMap::new(),
        traits: HashMap::new(),
        champion_icons: HashMap::new(),
        trait_icons: HashMap::new(),
        item_icons: HashMap::new(),
        item_names: HashMap::new(),
    };

    if let Some(set_data_list) = &content.set_data {
        for set_data in set_data_list {
            if let Some(champs) = &set_data.champions {
                for champ in champs {
                    if let Some(ref api_name) = champ.api_name {
                        let name = champ.name.clone().unwrap_or_default();
                        let icon = champ
                            .square_icon
                            .clone()
                            .or_else(|| champ.icon.clone())
                            .unwrap_or_default();
                        let api_lower = api_name.to_lowercase();
                        mapping.champions.insert(api_name.clone(), name.clone());
                        mapping.champions.insert(api_lower.clone(), name);

                        if !icon.is_empty() {
                            mapping
                                .champion_icons
                                .insert(api_name.clone(), icon.clone());
                            mapping.champion_icons.insert(api_lower, icon);
                        }
                    }
                }
            }
            if let Some(traits) = &set_data.traits {
                for trait_data in traits {
                    if let Some(ref api_name) = trait_data.api_name {
                        let name = trait_data.name.clone().unwrap_or_default();
                        let icon = trait_data.icon.clone().unwrap_or_default();
                        let api_lower = api_name.to_lowercase();
                        mapping.traits.insert(api_name.clone(), name.clone());
                        mapping.traits.insert(api_lower.clone(), name);

                        if !icon.is_empty() {
                            mapping.trait_icons.insert(api_name.clone(), icon.clone());
                            mapping.trait_icons.insert(api_lower, icon);
                        }
                    }
                }
            }
        }
    }

    if let Some(sets) = &content.sets {
        for set_data in sets.values() {
            if let Some(champs) = &set_data.champions {
                for champ in champs {
                    if let Some(ref api_name) = champ.api_name {
                        let name = champ.name.clone().unwrap_or_default();
                        let icon = champ
                            .square_icon
                            .clone()
                            .or_else(|| champ.icon.clone())
                            .unwrap_or_default();
                        let api_lower = api_name.to_lowercase();
                        mapping.champions.insert(api_name.clone(), name.clone());
                        mapping.champions.insert(api_lower.clone(), name);

                        if !icon.is_empty() {
                            mapping
                                .champion_icons
                                .insert(api_name.clone(), icon.clone());
                            mapping.champion_icons.insert(api_lower, icon);
                        }
                    }
                }
            }
            if let Some(traits) = &set_data.traits {
                for trait_data in traits {
                    if let Some(ref api_name) = trait_data.api_name {
                        let name = trait_data.name.clone().unwrap_or_default();
                        let icon = trait_data.icon.clone().unwrap_or_default();
                        let api_lower = api_name.to_lowercase();
                        mapping.traits.insert(api_name.clone(), name.clone());
                        mapping.traits.insert(api_lower.clone(), name);

                        if !icon.is_empty() {
                            mapping.trait_icons.insert(api_name.clone(), icon.clone());
                            mapping.trait_icons.insert(api_lower, icon);
                        }
                    }
                }
            }
        }
    }

    if let Some(items) = &content.items {
        for item in items {
            if let Some(ref api_name) = item.api_name {
                let api_lower = api_name.to_lowercase();
                if let Some(ref name) = item.name {
                    mapping.item_names.insert(api_name.clone(), name.clone());
                    mapping.item_names.insert(api_lower.clone(), name.clone());
                }
                if let Some(ref icon) = item.icon {
                    mapping.item_icons.insert(api_name.clone(), icon.clone());
                    mapping.item_icons.insert(api_lower, icon.clone());
                }
            }
        }
    }

    if let Some(augments) = &content.augments {
        for aug in augments {
            if let Some(ref api_name) = aug.api_name {
                let api_lower = api_name.to_lowercase();
                if let Some(ref name) = aug.name {
                    mapping.item_names.insert(api_name.clone(), name.clone());
                    mapping.item_names.insert(api_lower.clone(), name.clone());
                }
                if let Some(ref icon) = aug.icon {
                    mapping.item_icons.insert(api_name.clone(), icon.clone());
                    mapping.item_icons.insert(api_lower, icon.clone());
                }
            }
        }
    }

    mapping
}

fn get_tft_data_cache_path() -> Option<std::path::PathBuf> {
    let dir = dirs::config_dir()?.join("Yuumi").join("cache");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("tft_data.json"))
}

/// 抓取 TFT 基础数据字典（优先 LCU，备用 CDragon，带内存与磁盘缓存）
/// 参照 Seraphine：LCU → CDragon（一次，无重试，无代理）
async fn fetch_tft_data_mapping(lcu: &crate::LcuClient) -> TftDataMapping {
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

    let auth = build_auth_header(&lcu.token);
    let lcu_url = format!(
        "https://127.0.0.1:{}/lol-game-data/assets/v1/tft.json",
        lcu.port
    );

    // 参照 Seraphine checkAndUpdate：先尝试 LCU
    let lcu_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    if let Ok(resp) = lcu_client
        .get(&lcu_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(content) = resp.json::<TftJsonRoot>().await {
                let m = parse_tft_data(&content);
                if !m.champions.is_empty() {
                    *cache = Some(m.clone());
                    let m_clone = m.clone();
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

    // 2. 参照 Seraphine update 方法：LCU 失败后降级 CDragon，一次请求不重试
    let cdn_url = "https://raw.communitydragon.org/latest/cdragon/tft/zh_cn.json";
    let cdn_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    if let Ok(resp) = cdn_client.get(cdn_url).send().await {
        if resp.status().is_success() {
            if let Ok(content) = resp.json::<TftJsonRoot>().await {
                let m = parse_tft_data(&content);
                if !m.champions.is_empty() {
                    *cache = Some(m.clone());
                    let m_clone = m.clone();
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

// ─── Tauri 命令 ───

/// 从 LCU 获取 TFT 数据资源
#[tauri::command]
pub async fn get_tft_data(app_state: State<'_, AppState>) -> Result<TftDataMapping, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    Ok(fetch_tft_data_mapping(lcu).await)
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
    let tft_data = fetch_tft_data_mapping(lcu).await;

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
        let game_duration = g_obj
            .get("gameDuration")
            .and_then(|v| v.as_u64())
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
            .map(|dt| dt.format("%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "01-01 00:00".to_string());

        let duration_str = format!("{:02}:{:02}", game_duration / 60, game_duration % 60);

        let raw_participants = g_obj
            .get("participants")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut identity_map: HashMap<i64, (String, String)> = HashMap::new();
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
                        identity_map.insert(pid, (player_puuid, summoner_name));
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
            let pid_identity = identity_map.get(&pid);
            let parsed_p = parse_single_tft_participant(p_val, pid_identity, &tft_data, &puuid);
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
