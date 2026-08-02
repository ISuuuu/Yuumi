use crate::lcu::client::lcu_request;
use crate::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::MutexGuard;

/// SQLite 数据库文件路径：<config_dir>/Yuumi/saved_players.db
fn db_path() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("Yuumi");
    p.push("saved_players.db");
    p
}

/// 打开（必要时创建）数据库并建表
pub fn init_db() -> rusqlite::Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS saved_players (
            puuid TEXT NOT NULL,
            self_puuid TEXT NOT NULL,
            region TEXT NOT NULL DEFAULT '',
            rso_platform_id TEXT NOT NULL DEFAULT '',
            tag TEXT,
            summoner_name TEXT NOT NULL DEFAULT '',
            profile_icon_id INTEGER NOT NULL DEFAULT 0,
            update_at INTEGER NOT NULL,
            last_met_at INTEGER,
            PRIMARY KEY (puuid, self_puuid, region, rso_platform_id)
         );
         CREATE TABLE IF NOT EXISTS encountered_games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            game_id INTEGER NOT NULL,
            puuid TEXT NOT NULL,
            self_puuid TEXT NOT NULL,
            region TEXT NOT NULL DEFAULT '',
            rso_platform_id TEXT NOT NULL DEFAULT '',
            queue_type TEXT NOT NULL DEFAULT '',
            update_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_encountered_puuid
             ON encountered_games (puuid, self_puuid, queue_type);",
    )?;

    // 迁移：旧数据库补充 tag_line / champion_id 列
    let existing_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(saved_players)")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    if !existing_cols.iter().any(|c| c == "tag_line") {
        conn.execute_batch("ALTER TABLE saved_players ADD COLUMN tag_line TEXT;")?;
    }
    if !existing_cols.iter().any(|c| c == "champion_id") {
        conn.execute_batch(
            "ALTER TABLE saved_players ADD COLUMN champion_id INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // 迁移：将因此前数据采集 bug 误记录为 '0' 的对局模式默认全部更新为海克斯大乱斗 '2400'
    let _ = conn.execute(
        "UPDATE encountered_games SET queue_type = '2400' WHERE queue_type = '0';",
        [],
    );

    Ok(conn)
}

fn conn(state: &AppState) -> MutexGuard<'_, Connection> {
    state.saved_db.lock().unwrap_or_else(|e| e.into_inner())
}

fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ─── 数据结构 ───

/// 本局缓存：对局结束记录相遇时使用
#[derive(Debug, Clone)]
pub struct CurrentGameCache {
    pub game_id: i64,
    pub queue_id: i32,
    pub players: Vec<GamePlayerEntry>,
}

/// 单个玩家的对局信息
#[derive(Debug, Clone)]
pub struct GamePlayerEntry {
    pub puuid: String,
    pub summoner_name: String,
    pub profile_icon_id: i32,
    pub tag_line: Option<String>,
    pub champion_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlayerDto {
    pub puuid: String,
    pub self_puuid: String,
    pub region: String,
    pub rso_platform_id: String,
    pub tag: Option<String>,
    pub summoner_name: String,
    pub profile_icon_id: i32,
    #[serde(default)]
    pub tag_line: Option<String>,
    #[serde(default)]
    pub champion_id: i32,
    pub update_at: i64,
    pub last_met_at: Option<i64>,
    #[serde(default)]
    pub last_queue_type: Option<String>,
    #[serde(default)]
    pub encounter_count: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounteredGameDto {
    pub id: i64,
    pub game_id: i64,
    pub puuid: String,
    pub self_puuid: String,
    pub region: String,
    pub rso_platform_id: String,
    pub queue_type: String,
    pub update_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult<T> {
    pub data: Vec<T>,
    pub count: i64,
}

fn row_to_saved_player(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedPlayerDto> {
    Ok(SavedPlayerDto {
        puuid: row.get(0)?,
        self_puuid: row.get(1)?,
        region: row.get(2)?,
        rso_platform_id: row.get(3)?,
        tag: row.get(4)?,
        summoner_name: row.get(5)?,
        profile_icon_id: row.get(6)?,
        tag_line: row.get(7)?,
        champion_id: row.get(8)?,
        update_at: row.get(9)?,
        last_met_at: row.get(10)?,
        last_queue_type: row.get(11).ok().flatten(),
        encounter_count: row.get::<usize, i32>(12).unwrap_or(1),
    })
}

const SAVED_PLAYER_COLS: &str =
    "puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at";

fn row_to_encountered(row: &rusqlite::Row<'_>) -> rusqlite::Result<EncounteredGameDto> {
    Ok(EncounteredGameDto {
        id: row.get(0)?,
        game_id: row.get(1)?,
        puuid: row.get(2)?,
        self_puuid: row.get(3)?,
        region: row.get(4)?,
        rso_platform_id: row.get(5)?,
        queue_type: row.get(6)?,
        update_at: row.get(7)?,
    })
}

// ─── 对局结束自动记录相遇 ───

/// 记录一次对局相遇：upsert saved_player（更新 lastMetAt）+ 插入 encountered_games
pub fn record_encounter(
    state: &AppState,
    player: &GamePlayerEntry,
    self_puuid: &str,
    game_id: i64,
    queue_type: &str,
) {
    let conn = conn(state);
    let ts = now();

    // 主键含 region/rso_platform_id，若该玩家已存在则复用其值，
    // 否则用 '' 作为新的占位，避免同一玩家因 region 不一致产生重复行
    let (region, rso_platform_id): (String, String) = conn
        .query_row(
            "SELECT region, rso_platform_id FROM saved_players WHERE puuid = ?1 AND self_puuid = ?2",
            params![player.puuid, self_puuid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((String::new(), String::new()));

    if let Err(e) = conn.execute(
        "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at)
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, ?9)
         ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
           summoner_name = CASE WHEN excluded.summoner_name = '' THEN saved_players.summoner_name ELSE excluded.summoner_name END,
           profile_icon_id = CASE WHEN excluded.profile_icon_id = 0 THEN saved_players.profile_icon_id ELSE excluded.profile_icon_id END,
           tag_line = CASE WHEN excluded.tag_line IS NULL OR excluded.tag_line = '' THEN saved_players.tag_line ELSE excluded.tag_line END,
           champion_id = CASE WHEN excluded.champion_id = 0 THEN saved_players.champion_id ELSE excluded.champion_id END,
           update_at = excluded.update_at,
           last_met_at = excluded.last_met_at",
        params![
            player.puuid,
            self_puuid,
            region,
            rso_platform_id,
            player.summoner_name,
            player.profile_icon_id,
            player.tag_line,
            player.champion_id,
            ts
        ],
    ) {
        log::error!("记录相遇玩家失败: {}", e);
        return;
    }
    if let Err(e) = conn.execute(
        "INSERT INTO encountered_games (game_id, puuid, self_puuid, region, rso_platform_id, queue_type, update_at)
         VALUES (?1, ?2, ?3, '', '', ?4, ?5)",
        params![game_id, player.puuid, self_puuid, queue_type, ts],
    ) {
        log::error!("记录相遇对局失败: {}", e);
    }
}

/// 查询带 tag 的玩家（选人阶段聊天提醒使用），返回 (puuid, tag)
pub fn query_tagged_for_reminder(state: &AppState, self_puuid: &str) -> Vec<(String, String)> {
    let conn = conn(state);
    let mut stmt = match conn.prepare(
        "SELECT puuid, tag FROM saved_players WHERE self_puuid = ?1 AND tag IS NOT NULL AND tag != ''",
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("查询标记玩家失败: {}", e);
            return Vec::new();
        }
    };
    let rows = stmt.query_map(params![self_puuid], |r| Ok((r.get(0)?, r.get(1)?)));
    let out = match rows {
        Ok(iter) => iter.collect::<Result<Vec<_>, _>>().unwrap_or_default(),
        Err(e) => {
            log::error!("查询标记玩家失败: {}", e);
            Vec::new()
        }
    };
    out
}

// ─── Tauri 命令 ───

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSavedPlayerInput {
    pub puuid: String,
    pub self_puuid: String,
    #[serde(default)]
    pub rso_platform_id: String,
    #[serde(default)]
    pub region: String,
    pub tag: Option<String>,
    #[serde(default)]
    pub summoner_name: String,
    #[serde(default)]
    pub profile_icon_id: i32,
    #[serde(default)]
    pub encountered: bool,
}

/// 保存/更新玩家记录（upsert）。tag 为 None 时保留已有 tag，Some 时覆盖。
#[tauri::command]
pub fn save_saved_player(
    app_state: tauri::State<'_, AppState>,
    dto: SaveSavedPlayerInput,
) -> Result<(), String> {
    let conn = conn(&app_state);
    let ts = now();
    let last_met = if dto.encountered { Some(ts) } else { None };
    conn.execute(
        "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 0, ?8, ?9)
         ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
           tag = CASE WHEN excluded.tag IS NULL THEN saved_players.tag ELSE excluded.tag END,
           summoner_name = CASE WHEN excluded.summoner_name = '' THEN saved_players.summoner_name ELSE excluded.summoner_name END,
           profile_icon_id = CASE WHEN excluded.profile_icon_id = 0 THEN saved_players.profile_icon_id ELSE excluded.profile_icon_id END,
           update_at = excluded.update_at,
           last_met_at = COALESCE(excluded.last_met_at, saved_players.last_met_at)",
        params![
            dto.puuid,
            dto.self_puuid,
            dto.region,
            dto.rso_platform_id,
            dto.tag,
            dto.summoner_name,
            dto.profile_icon_id,
            ts,
            last_met,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 分页查询全部保存玩家（按最近相遇/更新时间倒序）
/// filter: "tagged" 只看已标记玩家，"multiple" 只看多次相遇玩家，其他为全部
#[tauri::command]
pub fn query_all_saved_players(
    app_state: tauri::State<'_, AppState>,
    self_puuid: String,
    page: Option<i64>,
    page_size: Option<i64>,
    filter: Option<String>,
) -> Result<PageResult<SavedPlayerDto>, String> {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(50).clamp(1, 200);
    let where_clause = match filter.as_deref() {
        Some("tagged") => " AND tag IS NOT NULL AND tag != ''",
        Some("multiple") => " AND (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = saved_players.puuid AND eg.self_puuid = saved_players.self_puuid) >= 2",
        _ => "",
    };
    let conn = conn(&app_state);
    let count: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM saved_players WHERE self_puuid = ?1{where_clause}"),
            params![self_puuid],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {}, \
             (SELECT queue_type FROM encountered_games eg WHERE eg.puuid = saved_players.puuid AND eg.self_puuid = saved_players.self_puuid ORDER BY eg.update_at DESC LIMIT 1), \
             (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = saved_players.puuid AND eg.self_puuid = saved_players.self_puuid) AS encounter_cnt \
             FROM saved_players WHERE self_puuid = ?1{where_clause} \
             ORDER BY last_met_at DESC, update_at DESC LIMIT ?2 OFFSET ?3",
            SAVED_PLAYER_COLS
        ))
        .map_err(|e| e.to_string())?;
    let data = stmt
        .query_map(
            params![self_puuid, page_size, (page - 1) * page_size],
            row_to_saved_player,
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(PageResult { data, count })
}

/// 保存玩家的精简标记（对局信息页徽章用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlayerMarker {
    pub tag: Option<String>,
    pub encounter_count: i32,
}

/// 获取全部保存玩家的精简映射：puuid → 标记信息（tag + 相遇次数）
#[tauri::command]
pub fn get_saved_players_map(
    app_state: tauri::State<'_, AppState>,
    self_puuid: String,
) -> Result<HashMap<String, SavedPlayerMarker>, String> {
    let conn = conn(&app_state);
    if self_puuid.is_empty() {
        return Ok(HashMap::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT sp.puuid,
                    sp.tag,
                    (SELECT COUNT(*) FROM encountered_games eg
                     WHERE eg.puuid = sp.puuid AND eg.self_puuid = sp.self_puuid)
             FROM saved_players sp WHERE sp.self_puuid = ?1",
        )
        .map_err(|e| e.to_string())?;
    let mut map = HashMap::new();
    let rows = stmt
        .query_map(params![self_puuid], |r| {
            Ok((
                r.get::<_, String>(0)?,
                SavedPlayerMarker {
                    tag: r.get(1)?,
                    encounter_count: r.get::<_, i32>(2).unwrap_or(1),
                },
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (puuid, marker) = row.map_err(|e| e.to_string())?;
        map.insert(puuid, marker);
    }
    Ok(map)
}

/// 分页查询与某玩家的相遇对局记录（按时间倒序）
#[tauri::command]
pub fn query_encountered_games(
    app_state: tauri::State<'_, AppState>,
    self_puuid: String,
    puuid: String,
    queue_type: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PageResult<EncounteredGameDto>, String> {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(1, 100);
    let conn = conn(&app_state);

    let q = queue_type.clone().unwrap_or_default();

    let count: i64 = if q.is_empty() {
        conn.query_row(
            "SELECT COUNT(*) FROM encountered_games WHERE self_puuid = ?1 AND puuid = ?2",
            params![self_puuid, puuid],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM encountered_games WHERE self_puuid = ?1 AND puuid = ?2 AND queue_type = ?3",
            params![self_puuid, puuid, q],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
    };

    let data = if q.is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT id, game_id, puuid, self_puuid, region, rso_platform_id, queue_type, update_at FROM encountered_games WHERE self_puuid = ?1 AND puuid = ?2 ORDER BY update_at DESC LIMIT ?3 OFFSET ?4",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params![self_puuid, puuid, page_size, (page - 1) * page_size],
                row_to_encountered,
            )
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, game_id, puuid, self_puuid, region, rso_platform_id, queue_type, update_at FROM encountered_games WHERE self_puuid = ?1 AND puuid = ?2 AND queue_type = ?3 ORDER BY update_at DESC LIMIT ?4 OFFSET ?5",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params![self_puuid, puuid, q, page_size, (page - 1) * page_size],
                row_to_encountered,
            )
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    Ok(PageResult { data, count })
}

/// 删除保存玩家及其相遇记录
#[tauri::command]
pub fn delete_saved_player(
    app_state: tauri::State<'_, AppState>,
    puuid: String,
    self_puuid: String,
) -> Result<(), String> {
    let conn = conn(&app_state);
    conn.execute(
        "DELETE FROM saved_players WHERE puuid = ?1 AND self_puuid = ?2",
        params![puuid, self_puuid],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM encountered_games WHERE puuid = ?1 AND self_puuid = ?2",
        params![puuid, self_puuid],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ─── 导出 / 导入 tag JSON ───

/// 回填历史保存玩家的 Riot ID（tagLine）：通过 LCU 按 puuid 查询召唤师信息补全 tag_line。
/// 返回更新的记录数。
#[tauri::command]
pub async fn backfill_saved_player_identity(
    app_state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    let app_state = app_state.inner();

    let targets: Vec<(String, String)> = {
        let conn = conn(app_state);
        let mut stmt = conn
            .prepare(
                "SELECT puuid, self_puuid FROM saved_players WHERE tag_line IS NULL OR tag_line = '' OR summoner_name IS NULL OR summoner_name = '' OR profile_icon_id = 0",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    if targets.is_empty() {
        return Ok(0);
    }

    use futures_util::StreamExt;

    // 1. 并发获取 LCU 数据，限流并发数为 8
    let mut stream = futures_util::stream::iter(targets)
        .map(|(puuid, self_puuid)| async move {
            if puuid.is_empty() {
                return None;
            }
            let path = format!("/lol-summoner/v2/summoners/puuid/{}", puuid);
            match lcu_request(app_state, "GET", &path, None).await {
                Ok(info) => {
                    let tag_line = info
                        .get("tagLine")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let summoner_name = info
                        .get("displayName")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .or_else(|| {
                            info.get("gameName")
                                .and_then(|v| v.as_str())
                                .filter(|s| !s.is_empty())
                        })
                        .unwrap_or("")
                        .to_string();
                    let profile_icon_id = info
                        .get("profileIconId")
                        .and_then(|v| v.as_i64())
                        .filter(|n| *n > 0)
                        .unwrap_or(0) as i32;

                    if tag_line.is_empty() && summoner_name.is_empty() && profile_icon_id == 0 {
                        None
                    } else {
                        Some((puuid, self_puuid, tag_line, summoner_name, profile_icon_id))
                    }
                }
                Err(_) => None,
            }
        })
        .buffer_unordered(8);

    let mut fetched_results = Vec::new();
    while let Some(res) = stream.next().await {
        if let Some(data) = res {
            fetched_results.push(data);
        }
    }

    // 2. 批量写入 SQLite (在一个事务中进行)
    let mut updated = 0u32;
    if !fetched_results.is_empty() {
        let mut conn = conn(app_state);
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        {
            let mut stmt = tx
                .prepare(
                    "UPDATE saved_players
                     SET tag_line = CASE WHEN tag_line IS NULL OR tag_line = '' THEN ?1 ELSE tag_line END,
                         summoner_name = CASE WHEN summoner_name IS NULL OR summoner_name = '' THEN ?2 ELSE summoner_name END,
                         profile_icon_id = CASE WHEN profile_icon_id = 0 THEN ?3 ELSE profile_icon_id END
                     WHERE puuid = ?4 AND self_puuid = ?5",
                )
                .map_err(|e| e.to_string())?;

            for (puuid, self_puuid, tag_line, summoner_name, profile_icon_id) in fetched_results {
                match stmt.execute(params![
                    tag_line,
                    summoner_name,
                    profile_icon_id,
                    puuid,
                    self_puuid
                ]) {
                    Ok(_) => updated += 1,
                    Err(e) => log::warn!("回填召唤师信息失败: {}", e),
                }
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
    }

    if updated > 0 {
        log::info!("已回填 {} 名保存玩家的身份信息", updated);
    }
    Ok(updated)
}

/// 导出所有带 tag 的玩家到用户选择的 JSON 文件。返回保存路径（取消则 None）。
#[tauri::command]
pub fn export_tagged_players_to_json_file(
    app_state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let conn = conn(&app_state);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} FROM saved_players WHERE tag IS NOT NULL AND tag != ''",
            SAVED_PLAYER_COLS
        ))
        .map_err(|e| e.to_string())?;
    let data = stmt
        .query_map([], row_to_saved_player)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let file = rfd::FileDialog::new()
        .set_title("导出标记玩家")
        .add_filter("JSON", &["json"])
        .set_file_name("tagged_players.json")
        .save_file();
    let Some(path) = file else {
        return Ok(None);
    };
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

/// 从用户选择的 JSON 文件导入标记玩家。返回导入数量（取消则 0）。
#[tauri::command]
pub fn import_tagged_players_from_json_file(
    app_state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    let file = rfd::FileDialog::new()
        .set_title("导入标记玩家")
        .add_filter("JSON", &["json"])
        .pick_file();
    let Some(path) = file else {
        return Ok(0);
    };
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let records: Vec<SavedPlayerDto> = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let conn = conn(&app_state);
    let mut count = 0u32;
    for r in records {
        let result = conn.execute(
            "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
               tag = excluded.tag,
               summoner_name = excluded.summoner_name,
               profile_icon_id = excluded.profile_icon_id,
               tag_line = excluded.tag_line,
               champion_id = excluded.champion_id,
               update_at = excluded.update_at,
               last_met_at = excluded.last_met_at",
            params![
                r.puuid,
                r.self_puuid,
                r.region,
                r.rso_platform_id,
                r.tag,
                r.summoner_name,
                r.profile_icon_id,
                r.tag_line,
                r.champion_id,
                r.update_at,
                r.last_met_at,
            ],
        );
        if let Err(e) = result {
            log::warn!("导入玩家失败: {}", e);
            continue;
        }
        count += 1;
    }
    Ok(count)
}
