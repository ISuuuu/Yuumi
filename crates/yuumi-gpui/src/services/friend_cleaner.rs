//! 好友清理服务（工单 D01）
//!
//! 对照 Vue `src/components/tools/FriendCleanerCard.vue` 与旧 Tauri `batch_delete_friends`：
//! - `GET /lol-chat/v1/friends` 读取真实好友列表
//! - 对离线好友查最近一场对局（`/lol-match-history/.../matches?begIndex=0&endIndex=1`），
//!   战绩为空/受限时用英雄熟练度 `lastPlayTime` 穿透，仍无结果则区分「战绩已隐藏 / 从未对局」
//! - 天数筛选、命中计数、搜索过滤全部基于上述真实数据
//! - 批量删除逐条 `DELETE /lol-chat/v1/friends/{id}`，按成功/失败数反馈

use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task::JoinSet;
use yuumi_core::lcu::client::LcuClient;

/// 单批并发检测的好友数量（对齐 Vue 的 chunkSize = 5）
const SCAN_CHUNK_SIZE: usize = 5;

/// 好友活跃状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendActivity {
    /// 在线 / 游戏中 / 离开
    Online,
    /// 查到最近一场对局
    Played,
    /// 战绩不可用时由英雄熟练度 `lastPlayTime` 穿透得到
    Mastery,
    /// 战绩被隐私保护隐藏
    Hidden,
    /// 从未打过游戏（也无熟练度记录）
    Never,
}

/// 好友清理列表条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanerFriend {
    pub id: String,
    pub puuid: String,
    pub name: String,
    pub note: String,
    pub icon: i64,
    pub availability: String,
    /// 最近活跃时间（毫秒时间戳）；在线好友取当前时间
    pub last_game_ms: Option<i64>,
    pub activity: FriendActivity,
}

impl CleanerFriend {
    /// 距最后一次对局的整天数；无有效时间（隐藏/从未）返回 None
    pub fn inactive_days(&self, now_ms: i64) -> Option<u32> {
        let ts = self.last_game_ms?;
        if ts <= 0 {
            return None;
        }
        let diff = now_ms.saturating_sub(ts);
        Some((diff / 86_400_000).max(0) as u32)
    }

    /// 是否命中指定筛选与搜索词（语义对齐 Vue `filteredFriends`）
    pub fn matches_filter(&self, filter: &str, query: &str, now_ms: i64) -> bool {
        let q = query.trim().to_lowercase();
        if !q.is_empty() {
            let name = self.name.to_lowercase();
            let note = self.note.to_lowercase();
            if !name.contains(&q) && !note.contains(&q) {
                return false;
            }
        }

        match filter {
            "all" => true,
            "hidden" => self.activity == FriendActivity::Hidden,
            "never" => self.activity == FriendActivity::Never,
            _ => {
                // 天数筛选绝对排除隐藏战绩的号，防止误删
                if self.activity == FriendActivity::Hidden {
                    return false;
                }
                let threshold = match filter {
                    "days360" => 360,
                    "days90" => 90,
                    _ => 180,
                };
                self.inactive_days(now_ms)
                    .map(|days| days >= threshold)
                    .unwrap_or(false)
            }
        }
    }

    /// 列表右侧的活跃时间文案（对齐 Vue `formatLastMatchTime`）
    pub fn activity_label(&self, now_ms: i64) -> String {
        match self.activity {
            FriendActivity::Online => "当前在线/游戏中".to_string(),
            FriendActivity::Hidden => "🔒 战绩已隐藏".to_string(),
            FriendActivity::Never => "从未对局 (0场战绩)".to_string(),
            FriendActivity::Played | FriendActivity::Mastery => match self.inactive_days(now_ms) {
                None => "无对局记录".to_string(),
                Some(0) => "今天曾对局".to_string(),
                Some(days) if days < 30 => format!("{} 天前对局", days),
                Some(days) if days < 365 => {
                    format!("{} 个月前 ({}天)", days / 30, days)
                }
                Some(days) => format!("{:.1} 年前 ({}天)", days as f64 / 365.0, days),
            },
        }
    }
}

/// 各筛选项命中数
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanerCounts {
    pub all: usize,
    pub days90: usize,
    pub days180: usize,
    pub days360: usize,
    pub never: usize,
    pub hidden: usize,
}

/// 统计各筛选项命中数（忽略搜索词，对齐 Vue `filterOptions`）
pub fn filter_counts(friends: &[CleanerFriend], now_ms: i64) -> CleanerCounts {
    let count = |filter: &str| {
        friends
            .iter()
            .filter(|f| f.matches_filter(filter, "", now_ms))
            .count()
    };
    CleanerCounts {
        all: friends.len(),
        days90: count("days90"),
        days180: count("days180"),
        days360: count("days360"),
        never: count("never"),
        hidden: count("hidden"),
    }
}

/// 当前筛选 + 搜索命中的好友
pub fn filtered_friends<'a>(
    friends: &'a [CleanerFriend],
    filter: &str,
    query: &str,
    now_ms: i64,
) -> Vec<&'a CleanerFriend> {
    friends
        .iter()
        .filter(|f| f.matches_filter(filter, query, now_ms))
        .collect()
}

/// 批量删除结果（部分删除语义）
#[derive(Debug, Clone, Default)]
pub struct CleanerDeleteOutcome {
    pub deleted: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// 扫描好友列表并检测每位离线好友的最后活跃时间
pub async fn scan_friends(client: &LcuClient) -> Result<Vec<CleanerFriend>, String> {
    let raw = client
        .get("/lol-chat/v1/friends")
        .await
        .map_err(|e| format!("获取好友列表失败: {}", e))?;
    let list = raw
        .as_array()
        .ok_or_else(|| "好友列表响应格式异常".to_string())?;

    let mut friends: Vec<CleanerFriend> = list.iter().map(parse_friend).collect();

    // 分批并发检测离线好友，避免一次性打满 LCU 并发许可
    let mut index = 0;
    while index < friends.len() {
        let end = (index + SCAN_CHUNK_SIZE).min(friends.len());
        let mut set = JoinSet::new();
        for (offset, friend) in friends[index..end].iter().enumerate() {
            let puuid = friend.puuid.clone();
            if puuid.is_empty() {
                continue;
            }
            let availability = friend.availability.clone();
            let client = client.clone();
            let i = index + offset;
            set.spawn(async move {
                let (activity, last_game_ms) =
                    classify_friend(&client, &puuid, &availability).await;
                (i, activity, last_game_ms)
            });
        }

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((i, activity, last_game_ms)) => {
                    if let Some(friend) = friends.get_mut(i) {
                        friend.activity = activity;
                        friend.last_game_ms = last_game_ms;
                    }
                }
                Err(e) => log::warn!("[FriendCleaner] 好友活跃时间检测任务失败: {}", e),
            }
        }

        index = end;
    }

    Ok(friends)
}

/// 批量删除好友，逐条累计成功与失败
pub async fn delete_friends(client: &LcuClient, ids: &[String]) -> CleanerDeleteOutcome {
    let mut outcome = CleanerDeleteOutcome::default();
    for id in ids {
        let path = format!("/lol-chat/v1/friends/{}", id);
        match client.delete(&path).await {
            Ok(_) => outcome.deleted.push(id.clone()),
            Err(e) => {
                log::warn!("[FriendCleaner] 删除好友 {} 失败: {}", id, e);
                outcome.failed.push((id.clone(), e));
            }
        }
    }
    outcome
}

/// 当前毫秒时间戳（失败时返回 0，仅影响天数展示）
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn parse_friend(value: &Value) -> CleanerFriend {
    let puuid = value
        .get("puuid")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| puuid.clone());
    let availability = value
        .get("availability")
        .and_then(|v| v.as_str())
        .unwrap_or("offline")
        .to_string();
    let name = friend_display_name(value);
    let note = value
        .get("note")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let icon = value.get("icon").and_then(|v| v.as_i64()).unwrap_or(0);

    let (activity, last_game_ms) = if availability != "offline" {
        (FriendActivity::Online, Some(now_ms()))
    } else {
        (FriendActivity::Never, None)
    };

    CleanerFriend {
        id,
        puuid,
        name,
        note,
        icon,
        availability,
        last_game_ms,
        activity,
    }
}

fn friend_display_name(value: &Value) -> String {
    let game_name = value.get("gameName").and_then(|v| v.as_str()).unwrap_or("");
    let game_tag = value.get("gameTag").and_then(|v| v.as_str()).unwrap_or("");
    if !game_name.is_empty() {
        if !game_tag.is_empty() {
            return format!("{}#{}", game_name, game_tag);
        }
        return game_name.to_string();
    }
    value
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "未知召唤师".to_string())
}

/// 判断好友活跃状态：在线 → 最近对局 → 熟练度穿透 → 隐藏/从未
async fn classify_friend(
    client: &LcuClient,
    puuid: &str,
    availability: &str,
) -> (FriendActivity, Option<i64>) {
    if availability != "offline" {
        return (FriendActivity::Online, Some(now_ms()));
    }

    let mut hidden_hint = false;
    let history_path = format!(
        "/lol-match-history/v1/products/lol/{}/matches?begIndex=0&endIndex=1",
        puuid
    );
    match client.get(&history_path).await {
        Ok(value) => {
            if let Some(ts) = first_game_time(&value) {
                return (FriendActivity::Played, Some(ts));
            }
        }
        Err(e) => {
            let upper = e.to_uppercase();
            hidden_hint = upper.contains("403")
                || upper.contains("PRIVACY")
                || upper.contains("PRIVATE")
                || upper.contains("FORBIDDEN");
        }
    }

    // 战绩为空/受限时用英雄熟练度 lastPlayTime 穿透真实最后游玩时间
    let mastery_path = format!("/lol-champion-mastery/v1/{}/champion-mastery", puuid);
    if let Ok(value) = client.get(&mastery_path).await {
        if let Some(ts) = max_last_play_time(&value) {
            return (FriendActivity::Mastery, Some(ts));
        }
    }

    if hidden_hint {
        (FriendActivity::Hidden, None)
    } else {
        (FriendActivity::Never, None)
    }
}

fn first_game_time(value: &Value) -> Option<i64> {
    let game = value
        .get("games")
        .and_then(|g| g.get("games"))
        .and_then(|g| g.as_array())
        .and_then(|games| games.first())?;
    if let Some(ts) = game.get("gameCreation").and_then(|v| v.as_i64()) {
        if ts > 0 {
            return Some(ts);
        }
    }
    game.get("gameCreationDate")
        .and_then(|v| v.as_str())
        .and_then(parse_iso_ms)
}

fn max_last_play_time(value: &Value) -> Option<i64> {
    value
        .as_array()?
        .iter()
        .filter_map(|entry| entry.get("lastPlayTime").and_then(|v| v.as_i64()))
        .max()
        .filter(|ts| *ts > 0)
}

fn parse_iso_ms(raw: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.timestamp_millis())
}
