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
}

// ─── 数据清洗 ───

impl LcuMatchGame {
    /// 将 LCU 原始对局数据清洗为前端展示结构
    pub fn to_display(&self, assets: &crate::lcu::game_data::GameDataAssets) -> MatchDisplay {
        let participant = &self.participants[0];
        let stats = &participant.stats;

        let cs = stats.total_minions_killed.unwrap_or(0)
            + stats.neutral_minions_killed.unwrap_or(0);
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

        let champion_icon_url =
            format!("/lol-game-data/assets/v1/champion-icons/{}.png", participant.champion_id);
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
        let rune_icon_url = assets
            .runes
            .get(&stats.perk0)
            .cloned()
            .unwrap_or_default();
        let item_icon_urls: Vec<String> = item_ids
            .iter()
            .filter(|&&id| id > 0)
            .filter_map(|id| assets.items.get(id).cloned())
            .collect();

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
        }
    }
}

// ─── 队列 ID 映射 ───

struct QueueInfo {
    name: &'static str,
    map: &'static str,
}

fn get_queue_info(queue_id: i32) -> QueueInfo {
    match queue_id {
        // 召唤师峡谷
        400 => QueueInfo { name: "征召模式", map: "召唤师峡谷" },
        420 => QueueInfo { name: "排位单双排", map: "召唤师峡谷" },
        430 => QueueInfo { name: "匹配模式", map: "召唤师峡谷" },
        440 => QueueInfo { name: "排位灵活组排", map: "召唤师峡谷" },
        490 => QueueInfo { name: "快速游戏", map: "召唤师峡谷" },
        // 嚎哭深渊
        450 => QueueInfo { name: "极地大乱斗", map: "嚎哭深渊" },
        // 海克斯大乱斗
        2400 => QueueInfo { name: "海克斯大乱斗", map: "嚎哭深渊" },
        // 限时/特殊模式
        800 => QueueInfo { name: "人机对战", map: "召唤师峡谷" },
        810 => QueueInfo { name: "人机对战", map: "召唤师峡谷" },
        820 => QueueInfo { name: "人机对战", map: "嚎哭深渊" },
        830 => QueueInfo { name: "人机对战", map: "召唤师峡谷" },
        840 => QueueInfo { name: "人机对战", map: "召唤师峡谷" },
        850 => QueueInfo { name: "人机对战", map: "召唤师峡谷" },
        900 => QueueInfo { name: "无限火力", map: "召唤师峡谷" },
        1010 => QueueInfo { name: "随机无限火力", map: "嚎哭深渊" },
        1020 => QueueInfo { name: "克隆模式", map: "召唤师峡谷" },
        1300 => QueueInfo { name: "极限闪击", map: "极限闪击" },
        1700 => QueueInfo { name: "斗魂竞技场", map: "斗魂竞技场" },
        1710 => QueueInfo { name: "斗魂竞技场", map: "斗魂竞技场" },
        // 捉鬼模式 (Swarm)
        1810 => QueueInfo { name: "捉鬼模式", map: "捉鬼模式" },
        1820 => QueueInfo { name: "捉鬼模式", map: "捉鬼模式" },
        1830 => QueueInfo { name: "捉鬼模式", map: "捉鬼模式" },
        1840 => QueueInfo { name: "捉鬼模式", map: "捉鬼模式" },
        // 自定义
        0 => QueueInfo { name: "自定义模式", map: "自定义" },
        _ => QueueInfo { name: "自定义模式", map: "自定义" },
    }
}

// ─── 时间工具函数 ───

/// 毫秒时间戳 → "2024-01-15 20:30"
fn timestamp_to_str(ms: u64) -> String {
    let secs = ms / 1000;
    // 简单实现：基于 Unix 时间戳计算
    // 使用 chrono 会更好，但为减少依赖，手动计算
    let (year, month, day, hour, min) = unix_secs_to_ymdhm(secs);
    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hour, min)
}

/// 毫秒时间戳 → "01-15 20:30"
fn timestamp_to_short_str(ms: u64) -> String {
    let secs = ms / 1000;
    let (_, month, day, hour, min) = unix_secs_to_ymdhm(secs);
    format!("{:02}-{:02} {:02}:{:02}", month, day, hour, min)
}

/// 秒数 → "25:30"
fn secs_to_str(total_secs: u64) -> String {
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

/// Unix 秒时间戳 → (年, 月, 日, 时, 分) — UTC
/// 注：简化实现，生产环境建议使用 chrono
pub fn unix_secs_to_ymdhm(secs: u64) -> (u32, u32, u32, u32, u32) {
    const SECS_PER_DAY: u64 = 86400;
    let days = secs / SECS_PER_DAY;
    let remaining = secs % SECS_PER_DAY;
    let hour = (remaining / 3600) as u32;
    let min = ((remaining % 3600) / 60) as u32;

    // 从 1970-01-01 开始推算日期
    let mut y = 1970u32;
    let mut d = days;
    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }

    let leap = is_leap_year(y);
    let days_in_month: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 1u32;
    for &dim in &days_in_month {
        if d < dim {
            break;
        }
        d -= dim;
        m += 1;
    }

    (y, m, (d + 1) as u32, hour, min)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
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
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let mut url = format!(
        "https://127.0.0.1:{}/lol-match-history/v1/products/lol/{}/matches",
        lcu.port, puuid
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

    let auth = build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("获取战绩失败: HTTP {}", resp.status()));
    }

    let history: LcuMatchHistoryResponse = resp.json().await.map_err(|e| e.to_string())?;

    let assets = app_state.game_data.read().await;
    let skip = beg_index.unwrap_or(0) as usize;
    let limit = match (beg_index, end_index) {
        (Some(b), Some(e)) if e > b => (e - b) as usize,
        _ => usize::MAX,
    };
    let displays: Vec<MatchDisplay> = history
        .games
        .games
        .iter()
        .filter(|g| !g.participants.is_empty())
        .skip(skip)
        .take(limit)
        .map(|g| g.to_display(&assets))
        .collect();

    Ok(displays)
}
