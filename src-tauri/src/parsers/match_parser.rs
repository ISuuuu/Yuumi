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
    #[serde(default)]
    pub timeline: Option<LcuParticipantTimeline>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LcuParticipantTimeline {
    #[serde(default)]
    pub lane: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
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
    #[serde(default)]
    pub role_bound_item: Option<i32>,
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
    // 召唤师峡谷等 5v5 模式的位置归一化（"TOP" | "JUNGLE" | "MIDDLE" | "BOTTOM" | "UTILITY"）
    #[serde(default)]
    pub position: Option<String>,
    // 官方分路任务/专属装备 ID
    #[serde(default)]
    pub role_bound_item: Option<i32>,
}

// ─── 数据清洗 ───

/// 判断是否为具备分路的召唤师峡谷类 5v5 模式
pub fn is_laned_mode(queue_id: i32) -> bool {
    matches!(
        queue_id,
        400 | 420
            | 430
            | 440
            | 480
            | 490
            | 700
            | 800
            | 810
            | 820
            | 830
            | 840
            | 850
            | 1300
            | 1400
            | 1900
    )
}

/// 根据官方分路任务/专属装备 ID 解析分路（TOP / MIDDLE / BOTTOM / UTILITY / JUNGLE）
pub fn resolve_role_bound_item_position(item_id: i32) -> Option<&'static str> {
    match item_id {
        // 上路任务与奖励道具（1090 任务: 上路, 1200 上路任务, 1220 解封的传送/上路奖励, 1221, 1222）
        1090 | 1200 | 1220 | 1221 | 1222 => Some("TOP"),
        // 中路任务与奖励道具（1091 任务: 中路, 1201 中路任务, 1206 中路奖励）
        1091 | 1201 | 1206 => Some("MIDDLE"),
        // 下路任务与奖励道具（1092 任务: 下路, 1202 下路任务, 1207 下路奖励）
        1092 | 1202 | 1207 => Some("BOTTOM"),
        // 辅助任务与奖励道具（1093 任务: 辅助, 1203 辅助任务, 1208 辅助奖励）
        1093 | 1203 | 1208 => Some("UTILITY"),
        // 打野任务与奖励道具（1094 任务: 打野, 1204 打野任务, 1205/1209/1210/1211 打野升级幼体）
        1094 | 1204 | 1205 | 1209 | 1210 | 1211 => Some("JUNGLE"),
        _ => None,
    }
}

/// 判断是否为辅助装备
pub fn is_support_item(item_id: i32) -> bool {
    matches!(item_id, 3850..=3871 | 3876..=3884)
}

/// 判断是否为打野专属装备/幼体
pub fn is_jungle_item(item_id: i32) -> bool {
    matches!(item_id, 1101..=1103)
}

/// 将 LCU 的 lane 和 role 推导为标准 5 峡谷位置（TOP, JUNGLE, MIDDLE, BOTTOM, UTILITY）
pub fn resolve_position(lane: Option<&str>, role: Option<&str>) -> Option<String> {
    let lane_clean = lane.map(|l| l.trim()).unwrap_or("");
    let role_clean = role.map(|r| r.trim()).unwrap_or("");

    let lane_upper = lane_clean.to_ascii_uppercase();
    let role_upper = role_clean.to_ascii_uppercase();

    // 1. 如果角色明确为辅助（SUPPORT / DUO_SUPPORT），不论 lane 是 BOTTOM 还是 NONE / 空，均归为 UTILITY
    if role_upper.contains("SUPPORT") {
        return Some("UTILITY".to_string());
    }

    // 2. 如果 lane 有具体值且不是 NONE
    if !lane_upper.is_empty() && lane_upper != "NONE" {
        match lane_upper.as_str() {
            "TOP" => return Some("TOP".to_string()),
            "JUNGLE" => return Some("JUNGLE".to_string()),
            "MIDDLE" | "MID" => return Some("MIDDLE".to_string()),
            "BOTTOM" | "BOT" => {
                if role_upper.contains("SUPPORT") {
                    return Some("UTILITY".to_string());
                } else {
                    return Some("BOTTOM".to_string());
                }
            }
            _ => {}
        }
    }

    // 3. 当 lane 为 NONE 或空时，从 role 进一步兜底
    if role_upper.contains("CARRY") || role_upper.contains("DUO") {
        return Some("BOTTOM".to_string());
    }

    None
}

/// 归一化 SGP teamPosition
pub fn normalize_sgp_position(val: Option<&str>) -> Option<String> {
    let val = val?.trim();
    if val.is_empty() || val.eq_ignore_ascii_case("NONE") {
        return None;
    }
    let upper = val.to_ascii_uppercase();
    match upper.as_str() {
        "TOP" => Some("TOP".to_string()),
        "JUNGLE" => Some("JUNGLE".to_string()),
        "MIDDLE" | "MID" => Some("MIDDLE".to_string()),
        "BOTTOM" | "BOT" => Some("BOTTOM".to_string()),
        "UTILITY" | "SUPPORT" => Some("UTILITY".to_string()),
        _ => None,
    }
}

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
    /// 将 LCU 原始对局数据清洗为前端展示结构。
    /// 无参与者时返回 None（不依赖调用方先做非空过滤，杜绝索引越界）。
    pub fn to_display(
        &self,
        assets: &crate::lcu::game_data::GameDataAssets,
    ) -> Option<MatchDisplay> {
        let participant = self.participants.first()?;
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

        let position = if !is_laned_mode(self.queue_id) {
            // 大乱斗（450/2400/2450）、斗魂竞技场（1700）等娱乐模式无分路
            None
        } else {
            // 1. 优先通过官方分路任务/专属装备 (roleBoundItem) 判定（最权威，彻底解决匹配/自选模式分路缺失与 timeline 误判）
            stats
                .role_bound_item
                .and_then(resolve_role_bound_item_position)
                .map(|pos| pos.to_string())
                // 2. 检查装备栏 (item0~item6) 是否含有专属分路任务装备
                .or_else(|| {
                    item_ids
                        .iter()
                        .find_map(|&id| resolve_role_bound_item_position(id))
                        .map(|pos| pos.to_string())
                })
                // 3. 惩戒技能（Smite, ID 11）必定为打野
                .or_else(|| {
                    if participant.spell1_id == 11 || participant.spell2_id == 11 {
                        Some("JUNGLE".to_string())
                    } else if item_ids.iter().any(|&id| is_support_item(id)) {
                        Some("UTILITY".to_string())
                    } else if item_ids.iter().any(|&id| is_jungle_item(id)) {
                        Some("JUNGLE".to_string())
                    } else {
                        None
                    }
                })
                // 4. LCU timeline (lane / role) 解析
                .or_else(|| {
                    participant
                        .timeline
                        .as_ref()
                        .and_then(|t| resolve_position(t.lane.as_deref(), t.role.as_deref()))
                })
        };

        Some(MatchDisplay {
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
            position,
            role_bound_item: stats.role_bound_item,
        })
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
        .filter_map(|g| g.to_display(&assets))
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
        let participant = participants.iter().find(|p| {
            p.get("puuid")
                .and_then(|v| v.as_str())
                .map(|p_str| p_str.eq_ignore_ascii_case(&puuid))
                .unwrap_or(false)
        });

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
        let perk0 = stats
            .get("perk0")
            .and_then(|v| v.as_i64())
            .or_else(|| {
                participant
                    .get("perks")
                    .and_then(|p| p.get("styles"))
                    .and_then(|s| s.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|s0| s0.get("selections"))
                    .and_then(|sel| sel.as_array())
                    .and_then(|sel_arr| sel_arr.first())
                    .and_then(|sel0| sel0.get("perk"))
                    .and_then(|v| v.as_i64())
            })
            .unwrap_or(0) as i32;
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

        let role_bound_item = stats
            .get("roleBoundItem")
            .or_else(|| participant.get("roleBoundItem"))
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);

        let position = if !is_laned_mode(queue_id) {
            // 大乱斗（450/2400/2450）、斗魂竞技场（1700）等娱乐模式无分路
            None
        } else {
            role_bound_item
                .and_then(resolve_role_bound_item_position)
                .map(|pos| pos.to_string())
                .or_else(|| {
                    item_ids
                        .iter()
                        .find_map(|&id| resolve_role_bound_item_position(id))
                        .map(|pos| pos.to_string())
                })
                .or_else(|| {
                    if spell1_id == 11 || spell2_id == 11 {
                        Some("JUNGLE".to_string())
                    } else if item_ids.iter().any(|&id| is_support_item(id)) {
                        Some("UTILITY".to_string())
                    } else if item_ids.iter().any(|&id| is_jungle_item(id)) {
                        Some("JUNGLE".to_string())
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    normalize_sgp_position(participant.get("teamPosition").and_then(|v| v.as_str()))
                })
                .or_else(|| {
                    normalize_sgp_position(
                        participant.get("selectedPosition").and_then(|v| v.as_str()),
                    )
                })
                .or_else(|| {
                    let lane = participant
                        .get("timeline")
                        .and_then(|t| t.get("lane"))
                        .and_then(|v| v.as_str());
                    let role = participant
                        .get("timeline")
                        .and_then(|t| t.get("role"))
                        .and_then(|v| v.as_str());
                    resolve_position(lane, role)
                })
        };

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
            position,
            role_bound_item,
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

        let d = game.to_display(&assets).unwrap();
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
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.kda, "Perfect");
        assert_eq!(d.name, "排位单双排");
        assert_eq!(d.map, "召唤师峡谷");
    }

    #[test]
    fn to_display_returns_none_without_participants() {
        let mut game = sample_game(420);
        game.participants.clear();
        assert!(game.to_display(&GameDataAssets::default()).is_none());
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

    #[test]
    fn resolve_position_maps_standard_lanes_and_roles() {
        assert_eq!(
            resolve_position(Some("TOP"), Some("SOLO")),
            Some("TOP".to_string())
        );
        assert_eq!(
            resolve_position(Some("JUNGLE"), Some("NONE")),
            Some("JUNGLE".to_string())
        );
        assert_eq!(
            resolve_position(Some("MIDDLE"), Some("SOLO")),
            Some("MIDDLE".to_string())
        );
        assert_eq!(
            resolve_position(Some("MID"), Some("SOLO")),
            Some("MIDDLE".to_string())
        );
        assert_eq!(
            resolve_position(Some("BOTTOM"), Some("DUO_CARRY")),
            Some("BOTTOM".to_string())
        );
        assert_eq!(
            resolve_position(Some("BOTTOM"), Some("DUO_SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some("BOT"), Some("SUPPORT")),
            Some("UTILITY".to_string())
        );
        // 关键场景：LCU 中排位/匹配的大量辅助玩家 lane 为 NONE 但 role 为 SUPPORT
        assert_eq!(
            resolve_position(Some("NONE"), Some("SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some(""), Some("SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some("NONE"), Some("DUO_SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some("NONE"), Some("DUO_CARRY")),
            Some("BOTTOM".to_string())
        );
        assert_eq!(resolve_position(Some("NONE"), Some("NONE")), None);
        assert_eq!(resolve_position(None, None), None);
        assert_eq!(resolve_position(Some(""), Some("")), None);
    }

    #[test]
    fn normalize_sgp_position_maps_positions() {
        assert_eq!(normalize_sgp_position(Some("TOP")), Some("TOP".to_string()));
        assert_eq!(
            normalize_sgp_position(Some("JUNGLE")),
            Some("JUNGLE".to_string())
        );
        assert_eq!(
            normalize_sgp_position(Some("MIDDLE")),
            Some("MIDDLE".to_string())
        );
        assert_eq!(
            normalize_sgp_position(Some("BOTTOM")),
            Some("BOTTOM".to_string())
        );
        assert_eq!(
            normalize_sgp_position(Some("UTILITY")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            normalize_sgp_position(Some("SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(normalize_sgp_position(Some("")), None);
        assert_eq!(normalize_sgp_position(Some("NONE")), None);
        assert_eq!(normalize_sgp_position(None), None);
    }

    #[test]
    fn to_display_extracts_position_from_timeline() {
        let mut game = sample_game(420);
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("BOTTOM".into()),
            role: Some("DUO_SUPPORT".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("UTILITY".to_string()));
    }

    #[test]
    fn to_display_extracts_support_when_lane_is_none() {
        let mut game = sample_game(420);
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("NONE".into()),
            role: Some("SUPPORT".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("UTILITY".to_string()));
    }

    #[test]
    fn to_display_extracts_jungle_from_smite_spell() {
        let mut game = sample_game(420);
        game.participants[0].spell1_id = 11; // Smite
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("NONE".into()),
            role: Some("NONE".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("JUNGLE".to_string()));
    }

    #[test]
    fn to_display_returns_none_position_for_aram_and_entertainment_modes() {
        // 极地大乱斗 450
        let mut game_aram = sample_game(450);
        game_aram.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("TOP".into()),
            role: Some("SOLO".into()),
        });
        let d_aram = game_aram.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d_aram.position, None);

        // 海克斯大乱斗 2400
        let mut game_hex = sample_game(2400);
        game_hex.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("BOTTOM".into()),
            role: Some("SUPPORT".into()),
        });
        let d_hex = game_hex.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d_hex.position, None);
    }

    #[test]
    fn resolve_role_bound_item_position_maps_all_classes() {
        assert_eq!(resolve_role_bound_item_position(1090), Some("TOP"));
        assert_eq!(resolve_role_bound_item_position(1200), Some("TOP"));
        assert_eq!(resolve_role_bound_item_position(1220), Some("TOP"));
        assert_eq!(resolve_role_bound_item_position(1091), Some("MIDDLE"));
        assert_eq!(resolve_role_bound_item_position(1206), Some("MIDDLE"));
        assert_eq!(resolve_role_bound_item_position(1092), Some("BOTTOM"));
        assert_eq!(resolve_role_bound_item_position(1207), Some("BOTTOM"));
        assert_eq!(resolve_role_bound_item_position(1093), Some("UTILITY"));
        assert_eq!(resolve_role_bound_item_position(1208), Some("UTILITY"));
        assert_eq!(resolve_role_bound_item_position(1094), Some("JUNGLE"));
        assert_eq!(resolve_role_bound_item_position(1209), Some("JUNGLE"));
        assert_eq!(resolve_role_bound_item_position(0), None);
        assert_eq!(resolve_role_bound_item_position(9999), None);
    }

    #[test]
    fn to_display_prioritizes_role_bound_item_over_erroneous_timeline() {
        let mut game = sample_game(430); // 430 匹配模式
        game.participants[0].champion_id = 68; // 兰博
        game.participants[0].stats.role_bound_item = Some(1220); // 官方上路任务道具/奖励
                                                                 // 关键复现场景：Riot 算法赛后将兰博误标为 lane: NONE, role: SUPPORT
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("NONE".into()),
            role: Some("SUPPORT".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        // 必须优先按专属任务道具 1220 正确推导为 TOP，而非被错误的 SUPPORT 误导为 UTILITY
        assert_eq!(d.position, Some("TOP".to_string()));
        assert_eq!(d.role_bound_item, Some(1220));
    }

    #[test]
    fn to_display_detects_role_bound_item_in_item_ids() {
        let mut game = sample_game(430);
        game.participants[0].champion_id = 517; // 塞拉斯
        game.participants[0].stats.item0 = 1206; // 中路任务奖励在装备栏中
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("NONE".into()),
            role: Some("NONE".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("MIDDLE".to_string()));
    }

    // ─── 响应快照（fixture）测试 ─────────────────────────────────────────────
    //
    // `src-tauri/testdata/match/` 存放 `/lol-match-history/v1/products/lol/{puuid}/matches`
    // 的**完整外层响应**：`real/` 是从真实客户端抓包（`pnpm fixtures:capture`），
    // `synthetic/` 是按 schema 合成、仅覆盖真实抓包缺失的场景。用途是防止结构体过度收紧，
    // 并锁定真实载荷下的分路 / 胜负判定行为。各样本覆盖的场景见同目录 manifest.json。

    struct Fixture {
        name: &'static str,
        raw: &'static str,
    }

    const REAL: &[Fixture] = &[
        Fixture {
            name: "real/top-solo",
            raw: include_str!("../../testdata/match/real/top-solo.json"),
        },
        Fixture {
            name: "real/mid-solo",
            raw: include_str!("../../testdata/match/real/mid-solo.json"),
        },
        Fixture {
            name: "real/jungle-none",
            raw: include_str!("../../testdata/match/real/jungle-none.json"),
        },
        Fixture {
            name: "real/bottom-carry",
            raw: include_str!("../../testdata/match/real/bottom-carry.json"),
        },
        Fixture {
            name: "real/bottom-support",
            raw: include_str!("../../testdata/match/real/bottom-support.json"),
        },
        Fixture {
            name: "real/none-duo",
            raw: include_str!("../../testdata/match/real/none-duo.json"),
        },
        Fixture {
            name: "real/remake",
            raw: include_str!("../../testdata/match/real/remake.json"),
        },
        Fixture {
            name: "real/custom-3100",
            raw: include_str!("../../testdata/match/real/custom-3100.json"),
        },
    ];

    const SYNTHETIC_ARAM: &str = include_str!("../../testdata/match/synthetic/aram-450.json");
    const SYNTHETIC_ARENA: &str = include_str!("../../testdata/match/synthetic/arena-1700.json");
    const SYNTHETIC_RANKED: &str = include_str!("../../testdata/match/synthetic/ranked-420.json");
    const SYNTHETIC_MINIMAL: &str = include_str!("../../testdata/match/synthetic/minimal-420.json");

    fn load_fixture(raw: &str) -> LcuMatchGame {
        let response: LcuMatchHistoryResponse =
            serde_json::from_str(raw).expect("fixture 应能按 LCU 外层响应反序列化");
        assert_eq!(response.games.games.len(), 1, "fixture 约定只含单局");
        response.games.games.into_iter().next().unwrap()
    }

    /// 诊断用：打印每份真实样本经当前规则算出的结果，便于核对断言。
    /// 手动执行 `cargo test --lib -- --ignored --nocapture dump_real_fixture_results`。
    #[test]
    #[ignore = "按需手动执行"]
    fn dump_real_fixture_results() {
        for fx in REAL {
            let game = load_fixture(fx.raw);
            let d = game.to_display(&GameDataAssets::default()).unwrap();
            println!(
                "{}: q={} win={} remake={} time={} dur={} kda={} cs={} gold={} dmg={} heal={} lvl={} champ={} rbi={:?} position={:?} items={:?}",
                fx.name,
                d.queue_id,
                d.win,
                d.remake,
                d.time,
                d.duration,
                d.kda,
                d.cs,
                d.gold,
                d.total_damage,
                d.total_heal,
                d.champ_level,
                d.champion_id,
                d.role_bound_item,
                d.position,
                d.item_ids,
            );
        }
    }

    #[test]
    fn fixture_ranked_420_parses_full_lcu_payload() {
        let game = load_fixture(SYNTHETIC_RANKED);
        let d = game.to_display(&GameDataAssets::default()).unwrap();

        assert_eq!(d.name, "排位单双排");
        assert_eq!(d.map, "召唤师峡谷");
        assert_eq!(d.time, "2024-01-15 14:30");
        assert_eq!(d.duration, "25:30");
        assert!(d.win);
        assert!(!d.remake);
        assert_eq!(d.kda, "5.33");
        assert_eq!(d.cs, 206);
        assert_eq!(d.role_bound_item, Some(1204));
        assert_eq!(d.position, Some("JUNGLE".to_string()));
        assert_eq!(d.item_ids, vec![6692, 3006, 6675, 6676, 1038, 3364, 3340]);
        assert!(d.item_icon_urls.is_empty());
    }

    #[test]
    fn fixture_aram_has_no_position_and_collects_augments() {
        let game = load_fixture(SYNTHETIC_ARAM);
        let d = game.to_display(&GameDataAssets::default()).unwrap();

        assert_eq!(d.name, "极地大乱斗");
        assert_eq!(d.map, "嚎哭深渊");
        assert_eq!(d.duration, "18:08");
        assert!(!d.win);
        assert_eq!(d.kda, "Perfect");
        assert_eq!(d.cs, 63);
        assert_eq!(d.position, None);
        assert_eq!(d.augment_ids, vec![7010, 7018, 7027, 7022]);
    }

    #[test]
    fn fixture_arena_exposes_subteam_placement() {
        let game = load_fixture(SYNTHETIC_ARENA);
        let d = game.to_display(&GameDataAssets::default()).unwrap();

        assert_eq!(d.name, "斗魂竞技场");
        assert_eq!(d.placement, Some(2));
        assert_eq!(d.position, None);
    }

    /// 稀疏载荷：只有结构体必填字段，其余全部缺失。
    /// 必须能解析且各项降级为 0 / None，不得 panic。
    #[test]
    fn fixture_minimal_payload_degrades_gracefully() {
        let game = load_fixture(SYNTHETIC_MINIMAL);
        let d = game.to_display(&GameDataAssets::default()).unwrap();

        assert_eq!(d.cs, 0);
        assert_eq!(d.gold, 0);
        assert_eq!(d.total_damage, 0);
        assert_eq!(d.total_heal, 0);
        assert_eq!(d.kda, "2.00");
        assert!(!d.remake);
        assert_eq!(d.placement, None);
        assert_eq!(d.position, None);
        assert_eq!(d.spell1_icon_url, "");
        assert_eq!(d.rune_icon_url, "");
        assert!(d.augment_ids.is_empty());
    }

    /// 真实样本的期望展示结果，逐份锁定。
    /// 这些值由真实抓包计算得出（见 `dump_real_fixture_results` 的输出），
    /// 任何解析规则改动导致行为变化都会在这里暴露。
    struct RealExpect {
        name: &'static str,
        queue_id: i32,
        win: bool,
        remake: bool,
        time: &'static str,
        duration: &'static str,
        kda: &'static str,
        cs: i32,
        gold: i32,
        total_damage: i32,
        total_heal: i32,
        champ_level: i32,
        champion_id: i32,
        role_bound_item: Option<i32>,
        position: Option<&'static str>,
        item_ids: [i32; 7],
    }

    const REAL_EXPECT: &[RealExpect] = &[
        RealExpect {
            name: "real/top-solo",
            queue_id: 430,
            win: true,
            remake: false,
            time: "2026-10-21 10:34",
            duration: "28:42",
            kda: "1.40",
            cs: 218,
            gold: 11827,
            total_damage: 14104,
            total_heal: 2636,
            champ_level: 17,
            champion_id: 17,
            role_bound_item: Some(1220),
            position: Some("TOP"),
            item_ids: [1056, 3115, 3006, 3087, 6653, 1028, 3364],
        },
        RealExpect {
            name: "real/mid-solo",
            queue_id: 430,
            win: true,
            remake: false,
            time: "2026-10-21 14:03",
            duration: "30:05",
            kda: "7.00",
            cs: 244,
            gold: 15590,
            total_damage: 29975,
            total_heal: 4646,
            champ_level: 17,
            champion_id: 17,
            role_bound_item: Some(1206),
            position: Some("MIDDLE"),
            item_ids: [1056, 3157, 3115, 3172, 6653, 3087, 3364],
        },
        RealExpect {
            // LCU 报 lane=JUNGLE，但 roleBoundItem=1220（上路任务奖励）且技能为传送而非惩戒，
            // 实际为上单。与 none-duo 同为「分路任务装备纠正 timeline」的真实案例。
            name: "real/jungle-none",
            queue_id: 430,
            win: true,
            remake: false,
            time: "2026-10-20 14:03",
            duration: "33:00",
            kda: "2.29",
            cs: 263,
            gold: 16626,
            total_damage: 36782,
            total_heal: 3498,
            champ_level: 18,
            champion_id: 17,
            role_bound_item: Some(1220),
            position: Some("TOP"),
            item_ids: [3115, 2420, 3087, 3006, 6653, 2510, 3364],
        },
        RealExpect {
            // roleBoundItem=3006（狂战士胫甲，非分路任务道具）无法判定分路，
            // 必须降级到 timeline 的 BOTTOM/CARRY 才能得到 BOTTOM。
            name: "real/bottom-carry",
            queue_id: 430,
            win: false,
            remake: false,
            time: "2026-10-20 13:17",
            duration: "41:28",
            kda: "1.31",
            cs: 239,
            gold: 19369,
            total_damage: 30158,
            total_heal: 9401,
            champ_level: 17,
            champion_id: 18,
            role_bound_item: Some(3006),
            position: Some("BOTTOM"),
            item_ids: [6676, 3026, 3031, 3072, 6675, 3035, 3363],
        },
        RealExpect {
            // roleBoundItem=2055（控制守卫，非任务道具）；装备栏含 3871（辅助装）→ UTILITY
            name: "real/bottom-support",
            queue_id: 430,
            win: true,
            remake: false,
            time: "2026-10-16 13:21",
            duration: "27:49",
            kda: "1.67",
            cs: 47,
            gold: 8150,
            total_damage: 17563,
            total_heal: 3599,
            champ_level: 13,
            champion_id: 63,
            role_bound_item: Some(2055),
            position: Some("UTILITY"),
            item_ids: [3871, 3020, 2031, 2503, 6653, 0, 3364],
        },
        RealExpect {
            // LCU lane=NONE / role=DUO，若只靠 role 兜底会被判成 BOTTOM。
            // 实际上单（已由用户核对），roleBoundItem=1220（上路任务奖励）+ 传送技能才是对的，
            // 说明分路任务装备的可靠性高于 LCU 的 timeline，当前优先级是正确的。
            name: "real/none-duo",
            queue_id: 430,
            win: false,
            remake: false,
            time: "2026-10-19 13:39",
            duration: "19:21",
            kda: "2.00",
            cs: 159,
            gold: 7786,
            total_damage: 16792,
            total_heal: 1941,
            champ_level: 14,
            champion_id: 17,
            role_bound_item: Some(1220),
            position: Some("TOP"),
            item_ids: [3006, 3087, 1026, 1056, 1043, 3108, 3364],
        },
        RealExpect {
            name: "real/remake",
            queue_id: 430,
            win: true,
            remake: true,
            time: "2026-10-21 13:33",
            duration: "01:13",
            kda: "Perfect",
            cs: 4,
            gold: 596,
            total_damage: 41,
            total_heal: 0,
            champ_level: 1,
            champion_id: 63,
            role_bound_item: Some(1201),
            position: Some("MIDDLE"),
            item_ids: [1056, 2003, 0, 0, 0, 0, 3340],
        },
        RealExpect {
            // 队列 3100 不在 is_laned_mode 白名单内 → 一律不推分路（即使 roleBoundItem 为 0、
            // timeline 给出 NONE/SUPPORT）。若要支持自定义对局分路，需先扩展该白名单。
            name: "real/custom-3100",
            queue_id: 3100,
            win: false,
            remake: false,
            time: "2026-10-19 14:51",
            duration: "08:07",
            kda: "2.50",
            cs: 24,
            gold: 3304,
            total_damage: 6821,
            total_heal: 1541,
            champ_level: 6,
            champion_id: 805,
            role_bound_item: Some(0),
            position: None,
            item_ids: [1001, 2031, 1052, 1056, 3057, 0, 3340],
        },
    ];

    #[test]
    fn real_fixtures_produce_expected_display() {
        assert_eq!(REAL.len(), REAL_EXPECT.len(), "样本数与期望表不一致");

        for (fx, exp) in REAL.iter().zip(REAL_EXPECT) {
            assert_eq!(fx.name, exp.name, "样本与期望表顺序不一致");
            let game = load_fixture(fx.raw);
            let d = game.to_display(&GameDataAssets::default()).unwrap();

            assert_eq!(d.queue_id, exp.queue_id, "{}: queueId", fx.name);
            assert_eq!(d.win, exp.win, "{}: win", fx.name);
            assert_eq!(d.remake, exp.remake, "{}: remake", fx.name);
            assert_eq!(d.time, exp.time, "{}: time", fx.name);
            assert_eq!(d.duration, exp.duration, "{}: duration", fx.name);
            assert_eq!(d.kda, exp.kda, "{}: kda", fx.name);
            assert_eq!(d.cs, exp.cs, "{}: cs", fx.name);
            assert_eq!(d.gold, exp.gold, "{}: gold", fx.name);
            assert_eq!(d.total_damage, exp.total_damage, "{}: 伤害", fx.name);
            assert_eq!(d.total_heal, exp.total_heal, "{}: 治疗", fx.name);
            assert_eq!(d.champ_level, exp.champ_level, "{}: 英雄等级", fx.name);
            assert_eq!(d.champion_id, exp.champion_id, "{}: 英雄", fx.name);
            assert_eq!(
                d.role_bound_item, exp.role_bound_item,
                "{}: roleBoundItem",
                fx.name
            );
            assert_eq!(d.position.as_deref(), exp.position, "{}: 分路", fx.name);
            assert_eq!(d.item_ids, exp.item_ids, "{}: 装备", fx.name);
            // 资源表为空时图标字段必须为空而不是 panic
            assert!(d.item_icon_urls.is_empty(), "{}: 图标", fx.name);
        }
    }

    /// 真实载荷前向兼容：stats 有 118 个字段、game 层有 gameMode / gameModeMutators /
    /// endOfGameResult / participantIdentities 等未建模字段，都必须被忽略而不是解析失败。
    #[test]
    fn real_fixtures_deserialize_and_ignore_unmodelled_fields() {
        for fx in REAL {
            let game = load_fixture(fx.raw);
            assert_eq!(
                game.participants.len(),
                1,
                "{}: current-summoner 的 matches 接口只返回本人",
                fx.name
            );
            assert!(game.game_id > 0, "{}: gameId 应存在", fx.name);
            assert!(game.to_display(&GameDataAssets::default()).is_some());
        }
    }

    /// 目录级守卫：`real/` 与 `synthetic/` 下新增任何样本都必须能被当前结构体反序列化。
    /// 这是发现「真实载荷多出必填字段 / 改了类型」最直接的手段。
    ///
    /// 只扫这两个目录，不递归 `testdata/match` 根：那里还会出现 `manifest.json`（非样本）、
    /// `_capture/`（`pnpm fixtures:capture` 的暂存区，含 gameflow / champ-select 等非
    /// matches 结构的会话快照），按本结构体解析必然失败。
    #[test]
    fn all_fixtures_on_disk_deserialize() {
        use std::path::Path;

        fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect(&path, out);
                } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    out.push(path);
                }
            }
        }

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/match");
        let mut files = Vec::new();
        for dir in ["real", "synthetic"] {
            let path = root.join(dir);
            assert!(path.is_dir(), "缺少 fixture 目录: {}", path.display());
            collect(&path, &mut files);
        }
        assert!(!files.is_empty(), "testdata/match 下未找到任何 fixture");

        for file in files {
            let raw = std::fs::read_to_string(&file).expect("读取 fixture 失败");
            let response: LcuMatchHistoryResponse = serde_json::from_str(&raw)
                .unwrap_or_else(|e| panic!("{} 无法反序列化: {e}", file.display()));
            assert!(
                !response.games.games.is_empty(),
                "{} 不含任何对局",
                file.display()
            );
        }
    }
}
