//! 路人集 SQLite 持久化层
//!
//! 兼容原有 Vue/Tauri 版数据表结构与 `%APPDATA%/Yuumi/saved_players.db` 文件，
//! 并补充置顶 (is_pinned) 与备忘录 (notes) 字段支持。

use crate::parsers::saved_player::{EncounteredGameRecord, SavedPlayer, TaggedPlayerExportDto};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 单个玩家在对局中的记录项（对局结束写入相遇时使用）
#[derive(Debug, Clone)]
pub struct GamePlayerRecord {
    pub puuid: String,
    pub summoner_name: String,
    pub tag_line: Option<String>,
    pub profile_icon_id: i32,
    pub champion_id: i32,
}

/// 默认数据库路径：<app_data_dir>/saved_players.db
pub fn default_db_path() -> PathBuf {
    crate::runtime::app_data_dir().join("saved_players.db")
}

/// 打开并迁移数据库
pub fn init_db(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let conn = Connection::open(path).map_err(|e| format!("打开数据库失败: {}", e))?;

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
            tag_line TEXT,
            champion_id INTEGER NOT NULL DEFAULT 0,
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
            ON encountered_games (puuid, self_puuid, queue_type);
         CREATE UNIQUE INDEX IF NOT EXISTS idx_encountered_unique
            ON encountered_games (game_id, puuid, self_puuid);",
    )
    .map_err(|e| format!("建表失败: {}", e))?;

    // 字段自动迁移检查
    let existing_saved_player_cols: Vec<String> = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(saved_players)")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok).collect()
    };

    if !existing_saved_player_cols.iter().any(|c| c == "tag_line") {
        let _ = conn.execute_batch("ALTER TABLE saved_players ADD COLUMN tag_line TEXT;");
    }
    if !existing_saved_player_cols
        .iter()
        .any(|c| c == "champion_id")
    {
        let _ = conn.execute_batch(
            "ALTER TABLE saved_players ADD COLUMN champion_id INTEGER NOT NULL DEFAULT 0;",
        );
    }
    if !existing_saved_player_cols.iter().any(|c| c == "is_pinned") {
        let _ = conn.execute_batch(
            "ALTER TABLE saved_players ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;",
        );
    }
    if !existing_saved_player_cols.iter().any(|c| c == "notes") {
        let _ = conn
            .execute_batch("ALTER TABLE saved_players ADD COLUMN notes TEXT NOT NULL DEFAULT '';");
    }

    let existing_game_cols: Vec<String> = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(encountered_games)")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok).collect()
    };
    if !existing_game_cols.iter().any(|c| c == "champion_id") {
        let _ = conn.execute_batch(
            "ALTER TABLE encountered_games ADD COLUMN champion_id INTEGER NOT NULL DEFAULT 0;",
        );
    }

    Ok(conn)
}

/// 线程安全的路人集数据存储句柄
#[derive(Clone)]
pub struct SavedPlayersStorage {
    conn: Arc<Mutex<Connection>>,
}

impl SavedPlayersStorage {
    /// 打开默认路径的数据存储
    pub fn open_default() -> Result<Self, String> {
        let path = default_db_path();
        Self::open(&path)
    }

    /// 打开指定路径的数据存储
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = init_db(path)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 查询玩家总数（用于分页与展示统计）
    pub fn count_players(
        &self,
        self_puuid: Option<&str>,
        filter: Option<&str>,
        search: Option<&str>,
    ) -> Result<u32, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let search_kw = search.unwrap_or_default().trim();
        let search_pattern = format!("%{}%", search_kw);

        let sql = "SELECT COUNT(*)
                   FROM saved_players sp
                   WHERE (?1 IS NULL OR ?1 = '' OR sp.self_puuid = ?1)
                     AND (?2 IS NULL OR ?2 = '' OR ?2 = '全部' OR ?2 = 'all'
                          OR (?2 = '置顶' AND sp.is_pinned = 1)
                          OR ((?2 = '已标记' OR ?2 = '标记玩家' OR ?2 = 'tagged') AND sp.tag IS NOT NULL AND sp.tag != '')
                          OR ((?2 = '多次相遇' OR ?2 = 'multiple') AND (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = sp.puuid AND eg.self_puuid = sp.self_puuid) >= 2)
                          OR sp.tag = ?2)
                     AND (?3 = '' OR sp.summoner_name LIKE ?4 OR COALESCE(sp.tag_line, '') LIKE ?4 OR COALESCE(sp.tag, '') LIKE ?4 OR COALESCE(sp.notes, '') LIKE ?4)";

        let count: i64 = conn
            .query_row(
                sql,
                params![self_puuid, filter, search_kw, search_pattern],
                |r| r.get(0),
            )
            .map_err(|e| format!("查询玩家总数失败: {}", e))?;

        Ok(count.max(0) as u32)
    }

    /// 分页查询玩家列表（支持按 self_puuid、tag 过滤与关键字搜索，带 limit/offset）
    pub fn query_players(
        &self,
        self_puuid: Option<&str>,
        filter: Option<&str>,
        search: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SavedPlayer>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let search_kw = search.unwrap_or_default().trim();
        let search_pattern = format!("%{}%", search_kw);

        let sql = "SELECT sp.puuid, sp.summoner_name, COALESCE(sp.tag_line, ''), sp.tag,
                          sp.champion_id, sp.profile_icon_id,
                          (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = sp.puuid AND eg.self_puuid = sp.self_puuid) AS enc_cnt,
                          COALESCE(sp.last_met_at, sp.update_at) AS last_met,
                          sp.is_pinned, COALESCE(sp.notes, ''),
                          (SELECT queue_type FROM encountered_games eg WHERE eg.puuid = sp.puuid AND eg.self_puuid = sp.self_puuid ORDER BY eg.update_at DESC LIMIT 1) AS last_queue,
                          sp.self_puuid
                   FROM saved_players sp
                   WHERE (?1 IS NULL OR ?1 = '' OR sp.self_puuid = ?1)
                     AND (?2 IS NULL OR ?2 = '' OR ?2 = '全部' OR ?2 = 'all'
                          OR (?2 = '置顶' AND sp.is_pinned = 1)
                          OR ((?2 = '已标记' OR ?2 = '标记玩家' OR ?2 = 'tagged') AND sp.tag IS NOT NULL AND sp.tag != '')
                          OR ((?2 = '多次相遇' OR ?2 = 'multiple') AND (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = sp.puuid AND eg.self_puuid = sp.self_puuid) >= 2)
                          OR sp.tag = ?2)
                     AND (?3 = '' OR sp.summoner_name LIKE ?4 OR COALESCE(sp.tag_line, '') LIKE ?4 OR COALESCE(sp.tag, '') LIKE ?4 OR COALESCE(sp.notes, '') LIKE ?4)
                   ORDER BY sp.is_pinned DESC, last_met DESC, sp.update_at DESC
                   LIMIT ?5 OFFSET ?6";

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("准备查询语句失败: {}", e))?;

        let rows = stmt
            .query_map(
                params![
                    self_puuid,
                    filter,
                    search_kw,
                    search_pattern,
                    limit as i64,
                    offset as i64
                ],
                |r| {
                    let enc_count_i64: i64 = r.get(6)?;
                    let is_pinned_int: i32 = r.get(8)?;
                    let last_queue: Option<String> = r.get(10)?;

                    Ok(SavedPlayer {
                        puuid: r.get(0)?,
                        self_puuid: r.get(11)?,
                        summoner_name: r.get(1)?,
                        tag_line: r.get(2)?,
                        tag: r.get(3)?,
                        champion_id: r.get(4)?,
                        profile_icon_id: r.get(5)?,
                        encountered_count: enc_count_i64.max(1) as u32,
                        last_met_timestamp: r.get(7)?,
                        last_queue_type: last_queue,
                        is_pinned: is_pinned_int != 0,
                        notes: r.get(9)?,
                        encounter_history: Vec::new(),
                    })
                },
            )
            .map_err(|e| format!("执行查询失败: {}", e))?;

        let mut players = Vec::new();
        for row in rows {
            players.push(row.map_err(|e| format!("解析行数据失败: {}", e))?);
        }

        Ok(players)
    }

    /// 查询某玩家的历史相遇记录简讯
    pub fn query_encountered_games(
        &self,
        puuid: &str,
        self_puuid: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EncounteredGameRecord>, String> {
        self.query_encountered_games_paged(puuid, self_puuid, limit, 0)
    }

    /// 查询某玩家相遇记录总数
    pub fn count_encountered_games(
        &self,
        puuid: &str,
        self_puuid: Option<&str>,
    ) -> Result<u32, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let sql = "SELECT COUNT(*)
                   FROM encountered_games
                   WHERE puuid = ?1 AND (?2 IS NULL OR ?2 = '' OR self_puuid = ?2)";

        let count: i64 = conn
            .query_row(sql, params![puuid, self_puuid], |r| r.get(0))
            .map_err(|e| format!("查询相遇记录总数失败: {}", e))?;

        Ok(count.max(0) as u32)
    }

    /// 分页查询某玩家的历史相遇记录简讯
    pub fn query_encountered_games_paged(
        &self,
        puuid: &str,
        self_puuid: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<EncounteredGameRecord>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let sql = "SELECT game_id, queue_type, champion_id, update_at
                   FROM encountered_games
                   WHERE puuid = ?1 AND (?2 IS NULL OR ?2 = '' OR self_puuid = ?2)
                   ORDER BY update_at DESC
                   LIMIT ?3 OFFSET ?4";

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("准备对局历史查询失败: {}", e))?;

        let rows = stmt
            .query_map(
                params![puuid, self_puuid, limit as i64, offset as i64],
                |r| {
                    let game_id_i64: i64 = r.get(0)?;
                    Ok(EncounteredGameRecord {
                        game_id: game_id_i64 as u64,
                        queue_type: r.get(1)?,
                        champion_id: r.get(2)?,
                        timestamp: r.get(3)?,
                        result: None,
                    })
                },
            )
            .map_err(|e| format!("执行对局历史查询失败: {}", e))?;

        let mut list = Vec::new();
        for row in rows {
            list.push(row.map_err(|e| format!("解析对局历史行失败: {}", e))?);
        }

        Ok(list)
    }

    /// 导出所有带 tag 标记的玩家
    pub fn export_tagged_players(&self) -> Result<Vec<TaggedPlayerExportDto>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let sql = "SELECT puuid, self_puuid, region, rso_platform_id, tag, summoner_name,
                          profile_icon_id, tag_line, champion_id, update_at, last_met_at,
                          COALESCE(notes, '')
                   FROM saved_players
                   WHERE tag IS NOT NULL AND tag != ''";

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("准备导出查询失败: {}", e))?;

        let rows = stmt
            .query_map([], |r| {
                Ok(TaggedPlayerExportDto {
                    puuid: r.get(0)?,
                    self_puuid: r.get(1)?,
                    region: r.get(2)?,
                    rso_platform_id: r.get(3)?,
                    tag: r.get(4)?,
                    summoner_name: r.get(5)?,
                    profile_icon_id: r.get(6)?,
                    tag_line: r.get(7)?,
                    champion_id: r.get(8)?,
                    update_at: r.get(9)?,
                    last_met_at: r.get(10)?,
                    notes: r.get(11)?,
                })
            })
            .map_err(|e| format!("执行导出查询失败: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("解析导出数据行失败: {}", e))?);
        }

        Ok(result)
    }

    /// 批量导入标记玩家数据（upsert）
    pub fn import_tagged_players(&self, records: &[TaggedPlayerExportDto]) -> Result<u32, String> {
        if records.is_empty() {
            return Ok(0);
        }

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let tx = conn
            .transaction()
            .map_err(|e| format!("开启事务失败: {}", e))?;
        let mut count = 0u32;

        {
            let sql = "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at, notes)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                       ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
                         tag = excluded.tag,
                         summoner_name = CASE WHEN excluded.summoner_name = '' THEN saved_players.summoner_name ELSE excluded.summoner_name END,
                         profile_icon_id = CASE WHEN excluded.profile_icon_id = 0 THEN saved_players.profile_icon_id ELSE excluded.profile_icon_id END,
                         tag_line = CASE WHEN excluded.tag_line IS NULL OR excluded.tag_line = '' THEN saved_players.tag_line ELSE excluded.tag_line END,
                         champion_id = CASE WHEN excluded.champion_id = 0 THEN saved_players.champion_id ELSE excluded.champion_id END,
                         notes = CASE WHEN excluded.notes != '' THEN excluded.notes ELSE saved_players.notes END,
                         update_at = excluded.update_at,
                         last_met_at = COALESCE(excluded.last_met_at, saved_players.last_met_at)";

            let mut stmt = tx
                .prepare(sql)
                .map_err(|e| format!("准备导入语句失败: {}", e))?;

            for r in records {
                let res = stmt.execute(params![
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
                    r.notes,
                ]);
                if res.is_ok() {
                    count += 1;
                }
            }
        }

        tx.commit().map_err(|e| format!("提交事务失败: {}", e))?;
        Ok(count)
    }

    /// 切换或更新玩家置顶状态（带完整本机账号与目标记录作用域）
    pub fn update_pin(&self, puuid: &str, self_puuid: &str, is_pinned: bool) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let pinned_val = if is_pinned { 1 } else { 0 };
        let now = chrono::Utc::now().timestamp_millis();

        let rows = conn.execute(
            "UPDATE saved_players SET is_pinned = ?1, update_at = ?2 WHERE puuid = ?3 AND self_puuid = ?4",
            params![pinned_val, now, puuid, self_puuid],
        )
        .map_err(|e| format!("更新置顶失败: {}", e))?;

        if rows == 0 {
            return Err(format!(
                "未找到目标玩家记录进行置顶更新 (puuid={}, self_puuid={})",
                puuid, self_puuid
            ));
        }

        Ok(())
    }

    /// 更新玩家标签与备忘录（带完整本机账号与目标记录作用域，避免空账号跨记录）
    pub fn update_tag_and_notes(
        &self,
        puuid: &str,
        self_puuid: &str,
        tag: Option<&str>,
        notes: &str,
    ) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let now = chrono::Utc::now().timestamp_millis();

        let rows = conn.execute(
            "UPDATE saved_players SET tag = ?1, notes = ?2, update_at = ?3 WHERE puuid = ?4 AND self_puuid = ?5",
            params![tag, notes, now, puuid, self_puuid],
        )
        .map_err(|e| format!("更新玩家标签与备注失败: {}", e))?;

        if rows == 0 {
            return Err(format!(
                "未找到目标玩家记录进行标签更新 (puuid={}, self_puuid={})",
                puuid, self_puuid
            ));
        }

        Ok(())
    }

    /// 删除玩家记录以及关联的相遇历史（带完整本机账号与目标记录作用域）
    pub fn delete_player(&self, puuid: &str, self_puuid: &str) -> Result<(), String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let rows = tx
            .execute(
                "DELETE FROM saved_players WHERE puuid = ?1 AND self_puuid = ?2",
                params![puuid, self_puuid],
            )
            .map_err(|e| e.to_string())?;

        tx.execute(
            "DELETE FROM encountered_games WHERE puuid = ?1 AND self_puuid = ?2",
            params![puuid, self_puuid],
        )
        .map_err(|e| e.to_string())?;

        if rows == 0 {
            return Err(format!(
                "未找到目标玩家记录进行删除 (puuid={}, self_puuid={})",
                puuid, self_puuid
            ));
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 批量记录对局相遇玩家（对局结束时调用，自动按 (game_id, puuid, self_puuid) 去重）
    pub fn record_encounters(
        &self,
        players: &[GamePlayerRecord],
        self_puuid: &str,
        game_id: u64,
        queue_type: &str,
    ) -> Result<usize, String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let ts = chrono::Utc::now().timestamp_millis();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut recorded = 0;

        for player in players {
            if player.puuid.is_empty() || player.puuid == self_puuid {
                continue;
            }

            // 保持与 Vue/Tauri 版本的 ON CONFLICT 兼容
            let (region, rso_platform_id): (String, String) = tx
                .query_row(
                    "SELECT region, rso_platform_id FROM saved_players WHERE puuid = ?1 AND self_puuid = ?2",
                    params![player.puuid, self_puuid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap_or((String::new(), String::new()));

            tx.execute(
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
                    ts,
                ],
            )
            .map_err(|e| format!("记录相遇玩家失败: {}", e))?;

            // 库内键防重复：同局同一玩家不重复插入
            let exists: bool = tx
                .query_row(
                    "SELECT 1 FROM encountered_games WHERE game_id = ?1 AND puuid = ?2 AND self_puuid = ?3 LIMIT 1",
                    params![game_id as i64, player.puuid, self_puuid],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if !exists {
                tx.execute(
                    "INSERT INTO encountered_games (game_id, puuid, self_puuid, region, rso_platform_id, queue_type, update_at, champion_id)
                     VALUES (?1, ?2, ?3, '', '', ?4, ?5, ?6)",
                    params![game_id as i64, player.puuid, self_puuid, queue_type, ts, player.champion_id],
                )
                .map_err(|e| format!("记录相遇对局失败: {}", e))?;
                recorded += 1;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(recorded)
    }

    /// 查询指定本机账号下所有已标记的玩家 (puuid, tag)，用于选人阶段标记提醒
    pub fn query_tagged_for_reminder(
        &self,
        self_puuid: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT puuid, tag FROM saved_players WHERE self_puuid = ?1 AND tag IS NOT NULL AND tag != ''",
            )
            .map_err(|e| format!("准备提醒查询失败: {}", e))?;

        let rows = stmt
            .query_map(params![self_puuid], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| format!("执行提醒查询失败: {}", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("解析提醒数据失败: {}", e))?);
        }
        Ok(out)
    }

    /// 获取指定本机账号的路人集简要标记映射 (puuid -> (tag, encounter_count))，用于对局信息页徽标展示
    pub fn get_saved_markers(
        &self,
        self_puuid: &str,
    ) -> std::collections::HashMap<String, (Option<String>, u32)> {
        let Ok(conn) = self.conn.lock() else {
            return std::collections::HashMap::new();
        };

        let sql = "SELECT sp.puuid, sp.tag,
                          (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = sp.puuid AND eg.self_puuid = sp.self_puuid) AS enc_cnt
                   FROM saved_players sp
                   WHERE (?1 = '' OR sp.self_puuid = ?1)
                     AND ((sp.tag IS NOT NULL AND sp.tag != '')
                          OR (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = sp.puuid AND eg.self_puuid = sp.self_puuid) >= 2)";

        let mut stmt = match conn.prepare(sql) {
            Ok(s) => s,
            Err(_) => return std::collections::HashMap::new(),
        };

        let rows = stmt.query_map(params![self_puuid], |r| {
            let puuid: String = r.get(0)?;
            let tag: Option<String> = r.get(1)?;
            let count: i64 = r.get(2)?;
            Ok((puuid, (tag, count.max(0) as u32)))
        });

        let mut map = std::collections::HashMap::new();
        if let Ok(rows) = rows {
            for r in rows.flatten() {
                map.insert(r.0, r.1);
            }
        }
        map
    }

    /// 查询缺少 tag_line、summoner_name 或 profile_icon_id 的记录，用于 LCU 身份信息异步补齐
    pub fn query_missing_identity_players(&self) -> Result<Vec<(String, String)>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT puuid, self_puuid FROM saved_players
                 WHERE tag_line IS NULL OR tag_line = '' OR summoner_name IS NULL OR summoner_name = '' OR profile_icon_id = 0",
            )
            .map_err(|e| format!("准备缺失身份查询失败: {}", e))?;

        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| format!("执行缺失身份查询失败: {}", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("解析缺失身份行失败: {}", e))?);
        }
        Ok(out)
    }

    /// 批量回填更新玩家身份信息 (puuid, self_puuid, tag_line, summoner_name, profile_icon_id)
    pub fn batch_update_identities(
        &self,
        updates: &[(String, String, String, String, i32)],
    ) -> Result<u32, String> {
        if updates.is_empty() {
            return Ok(0);
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut count = 0u32;
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

            for (puuid, self_puuid, tag_line, summoner_name, profile_icon_id) in updates {
                if stmt
                    .execute(params![
                        tag_line,
                        summoner_name,
                        profile_icon_id,
                        puuid,
                        self_puuid
                    ])
                    .is_ok()
                {
                    count += 1;
                }
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDb(PathBuf);
    impl TempDb {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "yuumi_test_{}_{}.db",
                name,
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ));
            let _ = std::fs::remove_file(&path);
            Self(path)
        }
    }
    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn test_saved_players_crud_flow() {
        let temp = TempDb::new("crud");
        let storage = SavedPlayersStorage::open(&temp.0).expect("初始化测试数据库失败");

        // 1. 模拟记录相遇
        let self_puuid = "test-self-puuid";
        let players = vec![
            GamePlayerRecord {
                puuid: "player-1".to_string(),
                summoner_name: "测试玩家一".to_string(),
                tag_line: Some("CN1".to_string()),
                profile_icon_id: 501,
                champion_id: 157,
            },
            GamePlayerRecord {
                puuid: "player-2".to_string(),
                summoner_name: "测试玩家二".to_string(),
                tag_line: Some("CN2".to_string()),
                profile_icon_id: 742,
                champion_id: 23,
            },
        ];

        let recorded = storage
            .record_encounters(&players, self_puuid, 999901, "420")
            .expect("记录相遇玩家失败");
        assert_eq!(recorded, 2);

        // 2. 查询玩家列表
        let list = storage
            .query_players(Some(self_puuid), None, None, 50, 0)
            .expect("查询玩家失败");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].encountered_count, 1);

        // 3. 更新置顶
        storage
            .update_pin("player-1", self_puuid, true)
            .expect("设置置顶失败");
        let list_after_pin = storage
            .query_players(Some(self_puuid), None, None, 50, 0)
            .expect("查询失败");
        assert_eq!(list_after_pin[0].puuid, "player-1");
        assert!(list_after_pin[0].is_pinned);

        // 4. 更新标签与备注
        storage
            .update_tag_and_notes("player-1", self_puuid, Some("大腿"), "中路很稳")
            .expect("更新标签失败");
        let tagged_list = storage
            .query_players(Some(self_puuid), Some("大腿"), None, 50, 0)
            .expect("标签筛选失败");
        assert_eq!(tagged_list.len(), 1);
        assert_eq!(tagged_list[0].notes, "中路很稳");

        // 5. 搜索
        let search_res = storage
            .query_players(Some(self_puuid), None, Some("很稳"), 50, 0)
            .expect("搜索失败");
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].puuid, "player-1");

        // 6. 相遇历史对局
        let games = storage
            .query_encountered_games("player-1", Some(self_puuid), 10)
            .expect("查询相遇对局失败");
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].game_id, 999901);
        assert_eq!(games[0].champion_id, 157);

        // 7. 删除玩家
        storage
            .delete_player("player-2", self_puuid)
            .expect("删除玩家失败");
        let list_after_del = storage
            .query_players(Some(self_puuid), None, None, 50, 0)
            .expect("查询失败");
        assert_eq!(list_after_del.len(), 1);
        assert_eq!(list_after_del[0].puuid, "player-1");

        // 8. 导出与导入
        let exported = storage.export_tagged_players().expect("导出失败");
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].puuid, "player-1");
        assert_eq!(exported[0].tag.as_deref(), Some("大腿"));

        let imported_count = storage.import_tagged_players(&exported).expect("导入失败");
        assert_eq!(imported_count, 1);

        // 9. 选人提醒查询测试
        let reminders = storage
            .query_tagged_for_reminder(self_puuid)
            .expect("提醒查询失败");
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].0, "player-1");
        assert_eq!(reminders[0].1, "大腿");

        // 10. 重复结算去重测试 (同局多次调用不重复计入)
        let duplicate_record = storage
            .record_encounters(&players[..1], self_puuid, 999901, "420")
            .expect("再次记录相遇应成功");
        assert_eq!(duplicate_record, 0, "已记录对局不应重复插入");
        let enc_games = storage
            .query_encountered_games("player-1", Some(self_puuid), 10)
            .expect("查询对局");
        assert_eq!(enc_games.len(), 1);

        // 11. 双账号隔离测试
        let other_self_puuid = "other-account-puuid";
        let other_players = vec![GamePlayerRecord {
            puuid: "player-1".to_string(),
            summoner_name: "测试玩家一".to_string(),
            tag_line: Some("CN1".to_string()),
            profile_icon_id: 501,
            champion_id: 157,
        }];
        storage
            .record_encounters(&other_players, other_self_puuid, 888801, "420")
            .expect("其他账号记录相遇");
        let my_enc = storage
            .query_encountered_games("player-1", Some(self_puuid), 10)
            .expect("我的相遇");
        assert_eq!(my_enc.len(), 1);
        assert_eq!(my_enc[0].game_id, 999901);

        let other_enc = storage
            .query_encountered_games("player-1", Some(other_self_puuid), 10)
            .expect("对方相遇");
        assert_eq!(other_enc.len(), 1);
        assert_eq!(other_enc[0].game_id, 888801);

        // 12. 身份回填测试
        let _missing = storage
            .query_missing_identity_players()
            .expect("查询缺失身份");
        // player-1 profile_icon_id 501, tag_line CN1, summoner_name 测试玩家一，都不缺失
        let updates = vec![(
            "player-1".to_string(),
            self_puuid.to_string(),
            "NEW_TAG".to_string(),
            "新名称".to_string(),
            999,
        )];
        let updated = storage.batch_update_identities(&updates).expect("更新身份");
        assert_eq!(updated, 1);
    }
}
