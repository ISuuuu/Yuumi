use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{build_auth_header, AppState};

// ─── LCU 原始响应结构体 ───

/// `/lol-match-history/v1/products/lol/{puuid}/matches` 的原始返回
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchHistoryResponse {
    pub games: LcuMatchGamesContainer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchGamesContainer {
    pub games: Vec<LcuMatchGame>,
    pub game_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchGame {
    pub game_id: u64,
    pub game_creation: u64,
    pub game_duration: u64,
    pub queue_id: i32,
    pub map_id: Option<u32>,
    pub participants: Vec<LcuMatchParticipant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchParticipant {
    pub champion_id: i32,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub stats: LcuMatchStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchStats {
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub champ_level: i32,
    pub item0: i32,
    pub item1: i32,
    pub item2: i32,
    pub item3: i32,
    pub item4: i32,
    pub item5: i32,
    pub item6: i32,
    pub perk0: i32,
    pub total_minions_killed: Option<i32>,
    pub neutral_minions_killed: Option<i32>,
    pub gold_earned: Option<i32>,
    pub total_damage_dealt_to_champions: Option<i32>,
    pub total_heal: Option<i32>,
    #[serde(default)]
    pub game_ended_in_early_surrender: bool,
    #[serde(default)]
    pub subteam_placement: Option<u32>,
    // 海克斯强化（海克斯大乱斗 queueId 2400 / 经典海斗 2450）
    #[serde(default)]
    pub augments: Vec<i32>,
    #[serde(default)]
    pub player_augment1: i32,
    #[serde(default)]
    pub player_augment2: i32,
    #[serde(default)]
    pub player_augment3: i32,
    #[serde(default)]
    pub player_augment4: i32,
    #[serde(default)]
    pub player_augment5: i32,
}

// ─── 前端展示用的清洗结构体 ───

/// 清洗后的单局战绩数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDisplay {
    pub queue_id: i32,
    pub game_id: u64,
    pub time: String,
    pub short_time: String,
    pub name: String,
    pub map: String,
    pub duration: String,
    pub remake: bool,
    pub win: bool,
    pub placement: Option<u32>,
    pub champion_id: i32,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub champ_level: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub kda: String,
    pub item_ids: Vec<i32>,
    pub rune_id: i32,
    pub cs: i32,
    pub gold: i32,
    pub time_stamp: u64,
    pub total_damage: i32,
    pub total_heal: i32,
    // 前端拼接图标的 URL 前缀
    pub champion_icon_url: String,
    pub spell1_icon_url: String,
    pub spell2_icon_url: String,
    pub rune_icon_url: String,
    pub item_icon_urls: Vec<String>,
    // 海克斯强化（仅海克斯大乱斗 2400 / 经典海斗 2450 有值）
    pub augment_ids: Vec<i32>,
    pub augment_icon_urls: Vec<String>,
    pub augment_names: Vec<String>,
}

// ─── 数据清洗 ───

/// 从 stats 中提取海克斯强化 ID，去重后最多返回 5 个。
/// 将候选海克斯强化 ID 去重、过滤 0、最多保留 5 个
fn dedupe_augment_ids<I: IntoIterator<Item = i32>>(ids: I) -> Vec<i32> {
    let mut seen = std::collections::HashSet::new();
    let mut ordered = Vec::new();
    for id in ids {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }
    ordered.truncate(5);
    ordered
}

/// 从 stats 中提取海克斯强化 ID（去重，最多 5 个）
fn extract_augment_ids(stats: &LcuMatchStats) -> Vec<i32> {
    dedupe_augment_ids(stats.augments.iter().copied().chain([
        stats.player_augment1,
        stats.player_augment2,
        stats.player_augment3,
        stats.player_augment4,
        stats.player_augment5,
    ]))
}

/// 根据 ID 列表从资源表解析海克斯图标/名称（名称为空时兜底"海克斯强化"）
fn resolve_augment_details(
    ids: &[i32],
    assets: &crate::lcu::game_data::GameDataAssets,
) -> (Vec<String>, Vec<String>) {
    let mut icon_urls = Vec::new();
    let mut names = Vec::new();
    for &id in ids {
        if let Some(detail) = assets.augments.get(&id) {
            icon_urls.push(detail.icon_path.clone());
            let name = if detail.name.trim().is_empty() {
                "海克斯强化".to_string()
            } else {
                detail.name.clone()
            };
            names.push(name);
        }
    }
    (icon_urls, names)
}

impl LcuMatchGame {
    /// 将 LCU 原始对局数据清洗为前端展示结构
    pub fn to_display(&self, assets: &crate::lcu::game_data::GameDataAssets) -> MatchDisplay {
        let participant = &self.participants[0];
        let stats = &participant.stats;

        let cs =
            stats.total_minions_killed.unwrap_or(0) + stats.neutral_minions_killed.unwrap_or(0);
        let gold = stats.gold_earned.unwrap_or(0);
        let total_damage = stats.total_damage_dealt_to_champions.unwrap_or(0);
        let total_heal = stats.total_heal.unwrap_or(0);

        let item_ids = vec![
            stats.item0,
            stats.item1,
            stats.item2,
            stats.item3,
            stats.item4,
            stats.item5,
            stats.item6,
        ];

        let queue_info = get_queue_info(self.queue_id);
        let time = timestamp_to_str(self.game_creation);
        let short_time = timestamp_to_short_str(self.game_creation);
        let duration = secs_to_str(self.game_duration);

        let kda = if stats.deaths == 0 {
            "Perfect".to_string()
        } else {
            format!(
                "{:.2}",
                (stats.kills as f64 + stats.assists as f64) / stats.deaths as f64
            )
        };

        let champion_icon_url = format!(
            "/lol-game-data/assets/v1/champion-icons/{}.png",
            participant.champion_id
        );
        let spell1_icon_url = assets
            .spells
            .get(&participant.spell1_id)
            .cloned()
            .unwrap_or_default();
        let spell2_icon_url = assets
            .spells
            .get(&participant.spell2_id)
            .cloned()
            .unwrap_or_default();
        let rune_icon_url = assets.runes.get(&stats.perk0).cloned().unwrap_or_default();
        let item_icon_urls: Vec<String> = item_ids
            .iter()
            .filter(|&&id| id > 0)
            .filter_map(|id| assets.items.get(id).cloned())
            .collect();

        let augment_ids = extract_augment_ids(stats);
        let (augment_icon_urls, augment_names) = resolve_augment_details(&augment_ids, assets);

        MatchDisplay {
            queue_id: self.queue_id,
            game_id: self.game_id,
            time,
            short_time,
            name: queue_info.name.to_string(),
            map: queue_info.map.to_string(),
            duration,
            remake: stats.game_ended_in_early_surrender,
            win: stats.win,
            placement: stats.subteam_placement,
            champion_id: participant.champion_id,
            spell1_id: participant.spell1_id,
            spell2_id: participant.spell2_id,
            champ_level: stats.champ_level,
            kills: stats.kills,
            deaths: stats.deaths,
            assists: stats.assists,
            kda,
            item_ids,
            rune_id: stats.perk0,
            cs,
            gold,
            time_stamp: self.game_creation,
            total_damage,
            total_heal,
            champion_icon_url,
            spell1_icon_url,
            spell2_icon_url,
            rune_icon_url,
            item_icon_urls,
            augment_ids,
            augment_icon_urls,
            augment_names,
        }
    }
}

// ─── 队列 ID 映射 ───

/// 将 queueId 映射为 OP.GG 使用的游戏模式标识（供自动选人与其他调用方复用）
pub fn queue_id_to_opgg_mode(queue_id: i32) -> &'static str {
    match queue_id {
        450 | 2400 | 2450 => "aram",
        1700 | 1710 => "arena",
        1300 => "nexus_blitz",
        900 | 1900 => "urf",
        _ => "ranked",
    }
}

struct QueueInfo {
    name: &'static str,
    map: &'static str,
}

fn get_queue_info(queue_id: i32) -> QueueInfo {
    match queue_id {
        // 召唤师峡谷
        400 => QueueInfo {
            name: "征召模式",
            map: "召唤师峡谷",
        },
        420 => QueueInfo {
            name: "排位单双排",
            map: "召唤师峡谷",
        },
        430 => QueueInfo {
            name: "匹配模式",
            map: "召唤师峡谷",
        },
        440 => QueueInfo {
            name: "排位灵活组排",
            map: "召唤师峡谷",
        },
        480 => QueueInfo {
            name: "快速模式",
            map: "召唤师峡谷",
        },
        490 => QueueInfo {
            name: "快速模式",
            map: "召唤师峡谷",
        },
        // 嚎哭深渊
        450 => QueueInfo {
            name: "极地大乱斗",
            map: "嚎哭深渊",
        },
        // 海克斯大乱斗
        2400 => QueueInfo {
            name: "海克斯大乱斗",
            map: "嚎哭深渊",
        },
        // 经典海斗 (Classic Hextech ARAM / KIWI_JADE)
        2450 => QueueInfo {
            name: "经典海斗",
            map: "嚎哭深渊",
        },
        // 限时/特殊模式
        800 => QueueInfo {
            name: "人机对战",
            map: "召唤师峡谷",
        },
        810 => QueueInfo {
            name: "人机对战",
            map: "召唤师峡谷",
        },
        820 => QueueInfo {
            name: "人机对战",
            map: "嚎哭深渊",
        },
        830 => QueueInfo {
            name: "人机对战",
            map: "召唤师峡谷",
        },
        840 => QueueInfo {
            name: "人机对战",
            map: "召唤师峡谷",
        },
        850 => QueueInfo {
            name: "人机对战",
            map: "召唤师峡谷",
        },
        900 => QueueInfo {
            name: "无限火力",
            map: "召唤师峡谷",
        },
        1010 => QueueInfo {
            name: "随机无限火力",
            map: "嚎哭深渊",
        },
        1020 => QueueInfo {
            name: "克隆模式",
            map: "召唤师峡谷",
        },
        1300 => QueueInfo {
            name: "极限闪击",
            map: "极限闪击",
        },
        1700 => QueueInfo {
            name: "斗魂竞技场",
            map: "斗魂竞技场",
        },
        1710 => QueueInfo {
            name: "斗魂竞技场",
            map: "斗魂竞技场",
        },
        // 捉鬼模式 (Swarm)
        1810 => QueueInfo {
            name: "捉鬼模式",
            map: "捉鬼模式",
        },
        1820 => QueueInfo {
            name: "捉鬼模式",
            map: "捉鬼模式",
        },
        1830 => QueueInfo {
            name: "捉鬼模式",
            map: "捉鬼模式",
        },
        1840 => QueueInfo {
            name: "捉鬼模式",
            map: "捉鬼模式",
        },
        // 经典模式 (League Classic)
        4300 => QueueInfo {
            name: "经典模式",
            map: "召唤师峡谷",
        },
        4310 => QueueInfo {
            name: "经典模式",
            map: "召唤师峡谷",
        },
        // 自定义
        0 => QueueInfo {
            name: "自定义模式",
            map: "自定义",
        },
        _ => QueueInfo {
            name: "自定义模式",
            map: "自定义",
        },
    }
}

// ─── 时间工具函数 ───

/// 毫秒时间戳 → "2024-01-15 20:30"
fn timestamp_to_str(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "1970-01-01 00:00".to_string())
}

/// 毫秒时间戳 → "01-15 20:30"
fn timestamp_to_short_str(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "01-01 00:00".to_string())
}

/// 秒数 → "25:30"
fn secs_to_str(total_secs: u64) -> String {
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

// ─── Tauri 命令 ───

/// 获取战绩列表（清洗后）
#[tauri::command]
pub async fn get_match_history(
    puuid: String,
    beg_index: Option<u32>,
    end_index: Option<u32>,
    app_state: State<'_, AppState>,
) -> Result<Vec<MatchDisplay>, String> {
    // 锁内只提取连接参数，尽早释放读锁，避免 HTTP 请求/资源等待期间阻塞 monitor 重连
    let (port, token, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let mut url = format!(
        "https://127.0.0.1:{}/lol-match-history/v1/products/lol/{}/matches",
        port, puuid
    );

    let mut params = Vec::new();
    if let Some(b) = beg_index {
        params.push(format!("begIndex={}", b));
    }
    if let Some(e) = end_index {
        params.push(format!("endIndex={}", e));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let auth = build_auth_header(&token);

    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("获取战绩失败: HTTP {}", resp.status()));
    }

    let history: LcuMatchHistoryResponse = resp.json().await.map_err(|e| e.to_string())?;

    // 如果资源尚未加载完成，且 LCU 已连接，进行等待以防止解析出来的图片/装备路径为空（最多等 5 秒）
    crate::lcu::client::wait_for_game_data(app_state.inner()).await;

    // 直接持读锁解析（to_display 为纯内存转换，无 await），避免每次全量深克隆 GameDataAssets
    let assets = app_state.game_data.read().await;
    let displays: Vec<MatchDisplay> = history
        .games
        .games
        .iter()
        .filter(|g| !g.participants.is_empty())
        .map(|g| g.to_display(&assets))
        .collect();

    Ok(displays)
}

/// 通过 SGP 接口获取战绩列表（支持分页，仅腾讯国服可用）
/// 类似 getSummonerGamesByPuuidViaSGP
#[tauri::command]
pub async fn get_match_history_sgp(
    puuid: String,
    beg_index: u32,
    end_index: u32,
    app_state: State<'_, AppState>,
) -> Result<Vec<MatchDisplay>, String> {
    // 锁内只提取连接参数，立即释放读锁，避免跨 SGP token 获取 + 15s 超时请求持有锁阻塞 monitor 重连写锁
    let (port, token, server) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (lcu.port, lcu.token.clone(), lcu.server.clone())
    };

    // 仅腾讯国服支持 SGP
    let Some(server) = server else {
        log::info!("无法获取服务器信息，跳过 SGP 战绩获取");
        return Ok(Vec::new());
    };
    let server_lower = server.to_lowercase();
    if !crate::lcu::sgp::is_tencent_server(&server_lower) {
        log::info!("非腾讯国服 ({})，跳过 SGP 战绩获取", server);
        return Ok(Vec::new());
    }

    let auth = build_auth_header(&token);

    // ── 1. 获取 SGP accessToken（30 分钟缓存复用）与共享客户端 ──
    let sgp_token = crate::lcu::sgp::get_sgp_token(port, &auth).await?;

    // ── 2. 构建 SGP base URL 与客户端 ──
    let sgp_base = crate::lcu::sgp::sgp_base_url(&server_lower);
    let sgp_client = crate::lcu::sgp::get_sgp_client();

    // ── 3. 请求 SGP 战绩接口（若 401 自动强制刷新 token 重试一次） ──
    if end_index < beg_index {
        return Err("参数错误: end_index 不能小于 beg_index".to_string());
    }
    let count = end_index - beg_index + 1;
    let sgp_url = format!(
        "{}/match-history-query/v1/products/lol/player/{}/SUMMARY",
        sgp_base, puuid
    );

    let mut current_token = sgp_token;
    let mut sgp_resp = sgp_client
        .get(&sgp_url)
        .header("Authorization", format!("Bearer {}", current_token))
        .query(&[
            ("startIndex", &beg_index.to_string()),
            ("count", &count.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("SGP 战绩请求失败: {}", e))?;

    // 遇到 401 时强制刷新 token 并重试一次
    if sgp_resp.status().as_u16() == 401 {
        log::warn!("SGP 战绩返回 401 未授权，尝试强制刷新 accessToken 重试");
        if let Ok(refreshed_token) = crate::lcu::sgp::get_sgp_token_force_refresh(port, &auth).await
        {
            current_token = refreshed_token;
            sgp_resp = sgp_client
                .get(&sgp_url)
                .header("Authorization", format!("Bearer {}", current_token))
                .query(&[
                    ("startIndex", &beg_index.to_string()),
                    ("count", &count.to_string()),
                ])
                .send()
                .await
                .map_err(|e| format!("SGP 战绩重试请求失败: {}", e))?;
        }
    }

    if !sgp_resp.status().is_success() {
        return Err(format!(
            "SGP 战绩返回错误: HTTP {}",
            sgp_resp.status().as_u16()
        ));
    }

    let sgp_data: serde_json::Value = sgp_resp
        .json()
        .await
        .map_err(|e| format!("解析 SGP 响应失败: {}", e))?;

    // ── 4. 解析 SGP 返回的对局数据 ──
    // SGP 返回格式: { "games": { "gameCount": N, "games": [{ "json": {...} }] } }
    // 或直接 { "games": [...] }
    let games = sgp_data
        .get("games")
        .and_then(|g| {
            if let Some(arr) = g.as_array() {
                Some(arr.clone())
            } else if let Some(inner) = g.get("games") {
                inner.as_array().cloned()
            } else {
                None
            }
        })
        .unwrap_or_default();

    if games.is_empty() {
        return Ok(Vec::new());
    }

    // 等待游戏资源加载完成
    crate::lcu::client::wait_for_game_data(app_state.inner()).await;

    let assets = app_state.game_data.read().await;
    let mut displays = Vec::new();

    for game_val in &games {
        // SGP 的游戏数据可能在 json 字段里
        let g = game_val.get("json").unwrap_or(game_val);

        let game_id = g.get("gameId").and_then(|v| v.as_u64()).unwrap_or(0);
        let game_creation = g.get("gameCreation").and_then(|v| v.as_u64()).unwrap_or(0);
        let game_duration = g
            .get("gameDuration")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(0);
        let queue_id = g.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let _map_id = g.get("mapId").and_then(|v| v.as_u64());

        // 找到当前玩家的参与数据（SGP 用 puuid 匹配）
        let participants = g
            .get("participants")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let participant = participants
            .iter()
            .find(|p| p.get("puuid").and_then(|v| v.as_str()) == Some(&puuid));

        let Some(participant) = participant else {
            continue;
        };

        let stats = participant.get("stats").unwrap_or(participant);

        let win = stats.get("win").and_then(|v| v.as_bool()).unwrap_or(false);
        let kills = stats.get("kills").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let deaths = stats.get("deaths").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let assists = stats.get("assists").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let champ_level = stats
            .get("champLevel")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let champion_id = participant
            .get("championId")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let spell1_id = participant
            .get("spell1Id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let spell2_id = participant
            .get("spell2Id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let perk0 = stats.get("perk0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let total_minions = stats
            .get("totalMinionsKilled")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            + stats
                .get("neutralMinionsKilled")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
        let gold = stats
            .get("goldEarned")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let total_damage = stats
            .get("totalDamageDealtToChampions")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let total_heal = stats.get("totalHeal").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let remake = stats
            .get("gameEndedInEarlySurrender")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let subteam_placement = stats
            .get("subteamPlacement")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let item0 = stats.get("item0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let item1 = stats.get("item1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let item2 = stats.get("item2").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let item3 = stats.get("item3").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let item4 = stats.get("item4").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let item5 = stats.get("item5").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let item6 = stats.get("item6").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let item_ids = vec![item0, item1, item2, item3, item4, item5, item6];
        let queue_info = get_queue_info(queue_id);

        let time = timestamp_to_str(game_creation);
        let short_time = timestamp_to_short_str(game_creation);
        let duration = secs_to_str(game_duration);

        // 从 stats 中提取海克斯强化 ID
        let augment_ids = dedupe_augment_ids(
            stats
                .get("augments")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_i64().map(|id| id as i32))
                .chain(
                    [
                        "playerAugment1",
                        "playerAugment2",
                        "playerAugment3",
                        "playerAugment4",
                        "playerAugment5",
                    ]
                    .into_iter()
                    .filter_map(|key| stats.get(key).and_then(|v| v.as_i64()).map(|id| id as i32)),
                ),
        );
        let (augment_icon_urls, augment_names) = resolve_augment_details(&augment_ids, &assets);

        let kda = if deaths == 0 {
            "Perfect".to_string()
        } else {
            format!("{:.2}", (kills as f64 + assists as f64) / deaths as f64)
        };

        let champion_icon_url = format!(
            "/lol-game-data/assets/v1/champion-icons/{}.png",
            champion_id
        );
        let spell1_icon_url = assets.spells.get(&spell1_id).cloned().unwrap_or_default();
        let spell2_icon_url = assets.spells.get(&spell2_id).cloned().unwrap_or_default();
        let rune_icon_url = assets.runes.get(&perk0).cloned().unwrap_or_default();
        let item_icon_urls: Vec<String> = item_ids
            .iter()
            .filter(|&&id| id > 0)
            .filter_map(|id| assets.items.get(id).cloned())
            .collect();

        displays.push(MatchDisplay {
            queue_id,
            game_id,
            time,
            short_time,
            name: queue_info.name.to_string(),
            map: queue_info.map.to_string(),
            duration,
            remake,
            win,
            placement: subteam_placement,
            champion_id,
            spell1_id,
            spell2_id,
            champ_level,
            kills,
            deaths,
            assists,
            kda,
            item_ids,
            rune_id: perk0,
            cs: total_minions as i32,
            gold,
            time_stamp: game_creation,
            total_damage,
            total_heal,
            champion_icon_url,
            spell1_icon_url,
            spell2_icon_url,
            rune_icon_url,
            item_icon_urls,
            augment_ids,
            augment_icon_urls,
            augment_names,
        });
    }

    Ok(displays)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTeammate {
    pub name: String,
    pub puuid: String,
    pub icon: String,
    pub total: u32,
    pub wins: u32,
    pub losses: u32,
    pub last_play_time: u64,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTeammatesResponse {
    pub puuid: String,
    pub summoners: Vec<RecentTeammate>,
}

struct TeammateInfo {
    name: String,
    puuid: String,
    icon: i32,
    win: bool,
}

struct GameTeammates {
    remake: bool,
    game_creation: u64,
    summoners: Vec<TeammateInfo>,
}

async fn fetch_game_teammates(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    game_id: u64,
    target_puuid: &str,
) -> Option<GameTeammates> {
    let url = format!("{}/lol-match-history/v1/games/{}", base, game_id);
    let resp = http
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .ok()?;
    let detail: serde_json::Value = resp.json().await.ok()?;

    let game_creation = detail
        .get("gameCreation")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let queue_id = detail.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0);
    let participants = detail.get("participants").and_then(|v| v.as_array())?;
    let identities = detail
        .get("participantIdentities")
        .and_then(|v| v.as_array())?;

    let mut target_pid: Option<i64> = None;

    // 1. 查找目标玩家的 participantId
    for ident in identities {
        let player_data = match ident.get("player") {
            Some(p) => p,
            None => continue,
        };
        let p_puuid = player_data
            .get("puuid")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if p_puuid == target_puuid {
            target_pid = ident.get("participantId").and_then(|v| v.as_i64());
            break;
        }
    }

    let target_pid = target_pid?;

    // 2. 查找目标玩家对应的 teamId 和这一局的 remake 状态
    let mut target_team: Option<i64> = None;
    let mut remake = false;

    for p in participants {
        let pid = match p.get("participantId").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => continue,
        };
        if pid == target_pid {
            let stats = match p.get("stats") {
                Some(s) => s,
                None => continue,
            };
            remake = stats
                .get("teamEarlySurrendered")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            target_team = if queue_id == 1700 {
                stats.get("subteamPlacement").and_then(|v| v.as_i64())
            } else {
                p.get("teamId").and_then(|v| v.as_i64())
            };
            break;
        }
    }

    let target_team = target_team?;

    // 3. 收集其他相同 teamId 的玩家作为队友
    let mut summoners = Vec::new();
    for p in participants {
        let pid = match p.get("participantId").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => continue,
        };
        if pid == target_pid {
            continue;
        }

        let p_team = if queue_id == 1700 {
            p.get("stats")
                .and_then(|s| s.get("subteamPlacement"))
                .and_then(|v| v.as_i64())
        } else {
            p.get("teamId").and_then(|v| v.as_i64())
        };

        if p_team == Some(target_team) {
            let p_win = p
                .get("stats")
                .and_then(|s| s.get("win"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            // 在 identities 中匹配玩家信息
            for ident in identities {
                let ident_pid = match ident.get("participantId").and_then(|v| v.as_i64()) {
                    Some(id) => id,
                    None => continue,
                };
                if ident_pid == pid {
                    let player_data = match ident.get("player") {
                        Some(p) => p,
                        None => continue,
                    };
                    let game_name = player_data
                        .get("gameName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let summoner_name = player_data
                        .get("summonerName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let display_name = player_data
                        .get("displayName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let mut name = if !game_name.is_empty() {
                        game_name.to_string()
                    } else if !summoner_name.is_empty() {
                        summoner_name.to_string()
                    } else {
                        display_name.to_string()
                    };

                    let tag_line = player_data
                        .get("tagLine")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !tag_line.is_empty() && !name.is_empty() {
                        name = format!("{}#{}", name, tag_line);
                    }
                    let puuid = player_data
                        .get("puuid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let icon = player_data
                        .get("profileIcon")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;

                    if !puuid.is_empty() && puuid != "00000000-0000-0000-0000-000000000000" {
                        summoners.push(TeammateInfo {
                            name,
                            puuid,
                            icon,
                            win: p_win,
                        });
                    }
                    break;
                }
            }
        }
    }

    Some(GameTeammates {
        remake,
        game_creation,
        summoners,
    })
}

#[tauri::command]
pub async fn get_recent_teammates(
    game_ids: Vec<u64>,
    puuid: String,
    app_state: State<'_, AppState>,
) -> Result<RecentTeammatesResponse, String> {
    // 锁内只提取连接参数，立即释放读锁，避免跨整个 fan-out await 持有锁阻塞 monitor 重连写锁
    let (auth, base, http) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().ok_or("LCU未连接")?;
        (
            build_auth_header(&lcu.token),
            format!("https://127.0.0.1:{}", lcu.port),
            lcu.http_client.clone(),
        )
    };

    // 复用 LCU 并发信号量，限制同时查询的对局数量，避免打满 LCU
    let semaphore = {
        let lock = app_state.api_semaphore.read().await;
        lock.clone()
    };

    let mut handles = Vec::new();
    for game_id in game_ids {
        let auth = auth.clone();
        let base = base.clone();
        let http = http.clone();
        let target_puuid = puuid.clone();
        let semaphore = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = match semaphore.acquire().await {
                Ok(p) => p,
                Err(_) => return None,
            };
            fetch_game_teammates(&http, &base, &auth, game_id, &target_puuid).await
        }));
    }

    let mut all_teammates = Vec::new();
    for h in handles {
        if let Ok(Some(res)) = h.await {
            all_teammates.push(res);
        }
    }

    // 查询被标记的玩家（tag 非空），标记玩家优先显示
    let tagged_map: std::collections::HashMap<String, String> =
        crate::saved_players::query_tagged_for_reminder(app_state.inner(), puuid.clone())
            .await
            .into_iter()
            .collect();

    // 统计队友
    let mut stats: std::collections::HashMap<String, RecentTeammate> =
        std::collections::HashMap::new();

    for game in all_teammates {
        for p in game.summoners {
            let entry = stats.entry(p.puuid.clone()).or_insert_with(|| {
                let icon_path = format!("/lol-game-data/assets/v1/profile-icons/{}.jpg", p.icon);
                RecentTeammate {
                    tag: tagged_map.get(&p.puuid).cloned(),
                    name: p.name,
                    puuid: p.puuid,
                    icon: icon_path,
                    total: 0,
                    wins: 0,
                    losses: 0,
                    last_play_time: game.game_creation,
                }
            });
            entry.total += 1;
            if !game.remake {
                if p.win {
                    entry.wins += 1;
                } else {
                    entry.losses += 1;
                }
            }
        }
    }

    // 查询被标记的玩家（tag 非空），标记玩家优先显示
    let mut summoners: Vec<RecentTeammate> = stats.into_values().collect();
    // 排序：标记玩家优先，其次按 total 降序
    summoners.sort_by(|a, b| {
        let a_tagged = tagged_map.contains_key(&a.puuid);
        let b_tagged = tagged_map.contains_key(&b.puuid);
        b_tagged.cmp(&a_tagged).then_with(|| b.total.cmp(&a.total))
    });
    // 取前 5 个
    summoners.truncate(5);

    Ok(RecentTeammatesResponse { puuid, summoners })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcu::game_data::{CherryAugmentDetail, GameDataAssets};
    use serde_json::json;

    #[test]
    fn dedupe_augment_ids_filters_zero_and_dedupes_in_order() {
        assert_eq!(dedupe_augment_ids([3, 0, 1, 3, 2, 1]), vec![3, 1, 2]);
        assert!(dedupe_augment_ids(std::iter::empty()).is_empty());
    }

    #[test]
    fn dedupe_augment_ids_truncates_to_five() {
        assert_eq!(
            dedupe_augment_ids([1, 2, 3, 4, 5, 6, 7]),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn extract_augment_ids_merges_augments_and_player_slots() {
        let stats: LcuMatchStats = serde_json::from_value(json!({
            "win": true, "kills": 0, "deaths": 0, "assists": 0,
            "champLevel": 18,
            "item0": 0, "item1": 0, "item2": 0, "item3": 0,
            "item4": 0, "item5": 0, "item6": 0, "perk0": 0,
            "augments": [7010, 0],
            "playerAugment1": 7018,
            "playerAugment2": 7027,
            "playerAugment3": 7010,
            "playerAugment4": 0,
            "playerAugment5": 7022
        }))
        .unwrap();
        assert_eq!(extract_augment_ids(&stats), vec![7010, 7018, 7027, 7022]);
    }

    #[test]
    fn queue_id_to_opgg_mode_maps_known_queues() {
        assert_eq!(queue_id_to_opgg_mode(450), "aram");
        assert_eq!(queue_id_to_opgg_mode(2400), "aram");
        assert_eq!(queue_id_to_opgg_mode(1700), "arena");
        assert_eq!(queue_id_to_opgg_mode(1300), "nexus_blitz");
        assert_eq!(queue_id_to_opgg_mode(900), "urf");
        assert_eq!(queue_id_to_opgg_mode(420), "ranked");
        assert_eq!(queue_id_to_opgg_mode(-1), "ranked");
    }

    fn sample_game(queue_id: i32) -> LcuMatchGame {
        serde_json::from_value(json!({
            "gameId": 483_920_751_u64,
            "gameCreation": 1_705_329_000_000_u64,
            "gameDuration": 1530_u64,
            "queueId": queue_id,
            "participants": [{
                "championId": 432,
                "spell1Id": 4,
                "spell2Id": 12,
                "stats": {
                    "win": true,
                    "kills": 10, "deaths": 2, "assists": 8,
                    "champLevel": 18,
                    "item0": 3157, "item1": 3020, "item2": 0, "item3": 0,
                    "item4": 0, "item5": 0, "item6": 0,
                    "perk0": 8112,
                    "totalMinionsKilled": 20,
                    "neutralMinionsKilled": 12,
                    "goldEarned": 13500,
                    "totalDamageDealtToChampions": 28000,
                    "totalHeal": 1200
                }
            }]
        }))
        .unwrap()
    }

    #[test]
    fn to_display_cleans_core_stats() {
        let mut assets = GameDataAssets::default();
        assets
            .spells
            .insert(4, "/lol-game-data/assets/spell/Summoner_Flash.png".into());
        assets
            .items
            .insert(3157, "/lol-game-data/assets/items/item_3157.png".into());
        assets.augments.insert(
            7018,
            CherryAugmentDetail {
                id: 7018,
                name: "".into(),
                icon_path: "/fe/lol-loot/aug_7018.png".into(),
            },
        );

        let mut game = sample_game(450);
        game.participants[0].stats.player_augment1 = 7018;

        let d = game.to_display(&assets);
        assert_eq!(d.name, "极地大乱斗");
        assert_eq!(d.map, "嚎哭深渊");
        assert_eq!(d.time, "2024-01-15 14:30");
        assert_eq!(d.duration, "25:30");
        assert!(d.win);
        assert!(!d.remake);
        assert_eq!(d.kda, "9.00");
        assert_eq!(d.cs, 32);
        assert_eq!(d.gold, 13500);
        assert_eq!(
            d.champion_icon_url,
            "/lol-game-data/assets/v1/champion-icons/432.png"
        );
        // 物品图标：仅 item0 有效（>0 且在资源表中）
        assert_eq!(d.item_icon_urls.len(), 1);
        assert_eq!(
            d.spell1_icon_url,
            "/lol-game-data/assets/spell/Summoner_Flash.png"
        );
        // 海克斯强化：名称为空时兜底
        assert_eq!(d.augment_names, vec!["海克斯强化"]);
        assert_eq!(d.augment_icon_urls, vec!["/fe/lol-loot/aug_7018.png"]);
    }

    #[test]
    fn to_display_perfect_kda_when_zero_deaths() {
        let mut game = sample_game(420);
        game.participants[0].stats.deaths = 0;
        let d = game.to_display(&GameDataAssets::default());
        assert_eq!(d.kda, "Perfect");
        assert_eq!(d.name, "排位单双排");
        assert_eq!(d.map, "召唤师峡谷");
    }

    #[test]
    fn secs_to_str_formats_minutes_and_seconds() {
        assert_eq!(secs_to_str(1530), "25:30");
        assert_eq!(secs_to_str(3725), "62:05"); // 不进位小时
        assert_eq!(secs_to_str(59), "00:59");
    }

    #[test]
    fn timestamp_helpers_format_utc() {
        assert_eq!(timestamp_to_str(1_705_329_000_000), "2024-01-15 14:30");
        assert_eq!(timestamp_to_short_str(1_705_329_000_000), "01-15 14:30");
        // 无效时间戳兜底
        assert_eq!(timestamp_to_str(u64::MAX), "1970-01-01 00:00");
    }
}
