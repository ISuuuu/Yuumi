use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// ─── LCU 原始响应结构体（与 src-tauri 严格对齐） ───

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
    #[serde(default)]
    pub perk0: i32,
    pub total_minions_killed: Option<i32>,
    pub neutral_minions_killed: Option<i32>,
    pub gold_earned: Option<i32>,
    pub total_damage_dealt_to_champions: Option<i32>,
    pub total_heal: Option<i32>,
    #[serde(default)]
    pub game_ended_in_early_surrender: bool,
    #[serde(default)]
    pub role_bound_item: Option<i32>,
}

// ─── 展示用战绩模型（对齐 YuumiX 紧凑排版） ───

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MatchItem {
    pub game_id: u64,
    pub queue_id: i32,
    pub queue_name: String,
    pub champion_id: i32,
    pub champion_name: String,
    pub champ_level: i32,
    pub win: bool,
    pub remake: bool,
    pub duration_str: String,
    pub time_str: String,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub kda_ratio: String,
    pub cs: i32,
    pub cs_per_minute: String,
    pub gold_str: String,
    pub damage_dealt: i32,
    pub damage_str: String,
    pub items: [i32; 7],
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub primary_rune_id: i32,
    pub position: Option<String>,
    pub expanded: bool,
}

// ─── 常用英雄 ID 到中文名称映射（内置常用列表，避免冷启动等待网络） ───

pub fn get_champion_name_zh(id: i32) -> &'static str {
    match id {
        1 => "安妮",
        2 => "奥拉夫",
        3 => "加里奥",
        4 => "卡牌大师",
        5 => "赵信",
        6 => "厄加特",
        7 => "乐芙兰",
        8 => "弗拉基米尔",
        9 => "费德提克",
        10 => "凯尔",
        11 => "易大师",
        12 => "阿利斯塔",
        13 => "瑞兹",
        14 => "赛恩",
        15 => "希维尔",
        16 => "索拉卡",
        17 => "提莫",
        18 => "崔丝塔娜",
        19 => "沃里克",
        20 => "努努和威朗普",
        21 => "厄运小姐",
        22 => "艾希",
        23 => "泰达米尔",
        24 => "贾克斯",
        25 => "莫甘娜",
        26 => "基兰",
        27 => "辛吉德",
        28 => "伊芙琳",
        29 => "图奇",
        30 => "卡尔萨斯",
        31 => "科加斯",
        32 => "阿木木",
        33 => "拉莫斯",
        34 => "艾尼维亚",
        35 => "萨科",
        36 => "蒙多医生",
        37 => "娑娜",
        38 => "卡萨丁",
        39 => "艾瑞莉娅",
        40 => "迦娜",
        41 => "普朗克",
        42 => "库奇",
        43 => "卡尔玛",
        44 => "塔里克",
        45 => "维迦",
        51 => "凯特琳",
        53 => "布里茨",
        54 => "墨菲特",
        55 => "卡特琳娜",
        64 => "李青",
        67 => "薇恩",
        81 => "伊泽瑞尔",
        84 => "阿卡丽",
        86 => "盖伦",
        92 => "锐雯",
        99 => "拉克丝",
        103 => "阿狸",
        104 => "格雷福斯",
        114 => "菲奥娜",
        141 => "凯隐",
        142 => "佐伊",
        145 => "卡莎",
        157 => "亚索",
        222 => "金克丝",
        235 => "赛娜",
        236 => "卢锡安",
        238 => "劫",
        350 => "悠米",
        412 => "锤石",
        429 => "卡莉丝塔",
        497 => "洛",
        498 => "霞",
        777 => "永恩",
        875 => "瑟提",
        897 => "奎桑提",
        _ => "英雄",
    }
}

pub fn get_queue_name_zh(queue_id: i32) -> &'static str {
    match queue_id {
        420 => "排位赛 单双排",
        440 => "排位赛 灵活排",
        430 => "匹配模式",
        450 => "极地大乱斗",
        490 => "快速匹配",
        1700 => "斗魂竞技场",
        2400 | 2450 => "海克斯大乱斗",
        3100 => "自定义对局",
        _ => "其他模式",
    }
}

pub fn resolve_position(lane: Option<&str>, role: Option<&str>) -> Option<String> {
    let lane_clean = lane.map(|l| l.trim()).unwrap_or("");
    let role_clean = role.map(|r| r.trim()).unwrap_or("");
    let lane_upper = lane_clean.to_ascii_uppercase();
    let role_upper = role_clean.to_ascii_uppercase();

    if role_upper.contains("SUPPORT") {
        return Some("UTILITY".to_string());
    }

    if !lane_upper.is_empty() && lane_upper != "NONE" {
        match lane_upper.as_str() {
            "TOP" => return Some("TOP".to_string()),
            "JUNGLE" => return Some("JUNGLE".to_string()),
            "MIDDLE" | "MID" => return Some("MIDDLE".to_string()),
            "BOTTOM" | "BOT" => {
                if role_upper.contains("SUPPORT") {
                    return Some("UTILITY".to_string())
                } else {
                    return Some("BOTTOM".to_string())
                }
            }
            _ => {}
        }
    }
    None
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

pub fn parse_fixture_game(game: &LcuMatchGame) -> Option<MatchItem> {
    let participant = game.participants.first()?;
    let stats = &participant.stats;

    // 重开判定：游戏时长 <= 300 秒且 early surrender
    let is_remake = game.game_duration <= 300 && stats.game_ended_in_early_surrender;

    let minutes = game.game_duration / 60;
    let seconds = game.game_duration % 60;
    let duration_str = format!("{:02}:{:02}", minutes, seconds);

    let k = stats.kills;
    let d = stats.deaths;
    let a = stats.assists;
    let kda_ratio = if d == 0 {
        format!("{:.1} 完美", (k + a) as f32)
    } else {
        format!("{:.2}:1", (k + a) as f32 / d as f32)
    };

    let total_cs = stats.total_minions_killed.unwrap_or(0) + stats.neutral_minions_killed.unwrap_or(0);
    let cs_per_min = if minutes > 0 {
        format!("{:.1}/m", total_cs as f32 / minutes as f32)
    } else {
        "0.0/m".to_string()
    };

    let gold = stats.gold_earned.unwrap_or(0);
    let gold_str = if gold >= 1000 {
        format!("{:.1}k", gold as f32 / 1000.0)
    } else {
        format!("{}", gold)
    };

    let damage = stats.total_damage_dealt_to_champions.unwrap_or(0);
    let damage_str = if damage >= 1000 {
        format!("{:.1}k", damage as f32 / 1000.0)
    } else {
        format!("{}", damage)
    };

    // 时间显示
    let dt: DateTime<Local> = Local
        .timestamp_millis_opt(game.game_creation as i64)
        .single()
        .unwrap_or_else(Local::now);
    let time_str = dt.format("%m-%d %H:%M").to_string();

    let items = [
        stats.item0,
        stats.item1,
        stats.item2,
        stats.item3,
        stats.item4,
        stats.item5,
        stats.item6,
    ];

    let position = resolve_position(
        participant.timeline.as_ref().and_then(|t| t.lane.as_deref()),
        participant.timeline.as_ref().and_then(|t| t.role.as_deref()),
    );

    Some(MatchItem {
        game_id: game.game_id,
        queue_id: game.queue_id,
        queue_name: get_queue_name_zh(game.queue_id).to_string(),
        champion_id: participant.champion_id,
        champion_name: get_champion_name_zh(participant.champion_id).to_string(),
        champ_level: stats.champ_level,
        win: stats.win,
        remake: is_remake,
        duration_str,
        time_str,
        kills: k,
        deaths: d,
        assists: a,
        kda_ratio,
        cs: total_cs,
        cs_per_minute: cs_per_min,
        gold_str,
        damage_dealt: damage,
        damage_str,
        items,
        spell1_id: participant.spell1_id,
        spell2_id: participant.spell2_id,
        primary_rune_id: stats.perk0,
        position,
        expanded: false,
    })
}

/// 从 real fixture 目录加载并组合成 20 条对局的真实历史
pub fn load_real_fixtures(testdata_dir: &Path) -> Vec<MatchItem> {
    let real_dir = testdata_dir.join("real");
    let fixture_files = [
        "top-solo.json",
        "mid-solo.json",
        "bottom-carry.json",
        "bottom-support.json",
        "jungle-none.json",
        "none-duo.json",
        "remake.json",
        "custom-3100.json",
    ];

    let mut single_games = Vec::new();
    for file in &fixture_files {
        let p = real_dir.join(file);
        if let Ok(content) = fs::read_to_string(&p) {
            if let Ok(resp) = serde_json::from_str::<LcuMatchHistoryResponse>(&content) {
                for game in resp.games.games {
                    if let Some(item) = parse_fixture_game(&game) {
                        single_games.push(item);
                    }
                }
            }
        }
    }

    if single_games.is_empty() {
        return Vec::new();
    }

    // 循环展开并生成 20 条具有细微时间差的对局列表以供虚拟滚动压测
    let mut result = Vec::with_capacity(20);
    for i in 0..20 {
        let mut base = single_games[i % single_games.len()].clone();
        base.game_id += i as u64;
        result.push(base);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_all_real_fixtures() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("src-tauri")
            .join("testdata")
            .join("match");
        let matches = load_real_fixtures(&root);
        assert_eq!(matches.len(), 20, "应加载并组合出 20 场真实战绩");
        let first = &matches[0];
        assert!(first.game_id > 0);
        assert!(!first.champion_name.is_empty());
    }
}
