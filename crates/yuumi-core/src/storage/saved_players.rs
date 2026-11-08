//! 路人集 SQLite 持久化层
//!
//! 兼容原有 Vue/Tauri 版数据表结构与 `%APPDATA%/Yuumi/saved_players.db` 文件，
//! 并补充置顶 (is_pinned) 与备忘录 (notes) 字段支持。

use crate::parsers::saved_player::{EncounteredGameRecord, SavedPlayer};
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
            ON encountered_games (puuid, self_puuid, queue_type);",
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
    if !existing_saved_player_cols.iter().any(|c| c == "champion_id") {
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
        let _ = conn.execute_batch(
            "ALTER TABLE saved_players ADD COLUMN notes TEXT NOT NULL DEFAULT '';",
        );
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
                   WHERE (?1 IS NULL OR sp.self_puuid = '' OR sp.self_puuid = ?1)
                     AND (?2 IS NULL OR ?2 = '' OR ?2 = '全部'
                          OR (?2 = '置顶' AND sp.is_pinned = 1)
                          OR (?2 = '已标记' AND sp.tag IS NOT NULL AND sp.tag != '')
                          OR (?2 = '多次相遇' AND (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = sp.puuid AND (sp.self_puuid = '' OR eg.self_puuid = sp.self_puuid)) >= 2)
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
                          (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = sp.puuid AND (sp.self_puuid = '' OR eg.self_puuid = sp.self_puuid)) AS enc_cnt,
                          COALESCE(sp.last_met_at, sp.update_at) AS last_met,
                          sp.is_pinned, COALESCE(sp.notes, '')
                   FROM saved_players sp
                   WHERE (?1 IS NULL OR sp.self_puuid = '' OR sp.self_puuid = ?1)
                     AND (?2 IS NULL OR ?2 = '' OR ?2 = '全部'
                          OR (?2 = '置顶' AND sp.is_pinned = 1)
                          OR (?2 = '已标记' AND sp.tag IS NOT NULL AND sp.tag != '')
                          OR (?2 = '多次相遇' AND (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = sp.puuid AND (sp.self_puuid = '' OR eg.self_puuid = sp.self_puuid)) >= 2)
                          OR sp.tag = ?2)
                     AND (?3 = '' OR sp.summoner_name LIKE ?4 OR COALESCE(sp.tag_line, '') LIKE ?4 OR COALESCE(sp.tag, '') LIKE ?4 OR COALESCE(sp.notes, '') LIKE ?4)
                   ORDER BY sp.is_pinned DESC, last_met DESC, sp.update_at DESC
                   LIMIT ?5 OFFSET ?6";

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("准备查询语句失败: {}", e))?;

        let rows = stmt
            .query_map(
                params![self_puuid, filter, search_kw, search_pattern, limit as i64, offset as i64],
                |r| {
                    let enc_count_i64: i64 = r.get(6)?;
                    let is_pinned_int: i32 = r.get(8)?;

                    Ok(SavedPlayer {
                        puuid: r.get(0)?,
                        summoner_name: r.get(1)?,
                        tag_line: r.get(2)?,
                        tag: r.get(3)?,
                        champion_id: r.get(4)?,
                        profile_icon_id: r.get(5)?,
                        encountered_count: enc_count_i64.max(1) as u32,
                        last_met_timestamp: r.get(7)?,
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let sql = "SELECT game_id, queue_type, champion_id, update_at
                   FROM encountered_games
                   WHERE puuid = ?1 AND (?2 IS NULL OR self_puuid = '' OR self_puuid = ?2)
                   ORDER BY update_at DESC
                   LIMIT ?3";

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("准备对局历史查询失败: {}", e))?;

        let rows = stmt
            .query_map(params![puuid, self_puuid, limit as i64], |r| {
                let game_id_i64: i64 = r.get(0)?;
                Ok(EncounteredGameRecord {
                    game_id: game_id_i64 as u64,
                    queue_type: r.get(1)?,
                    champion_id: r.get(2)?,
                    timestamp: r.get(3)?,
                    result: None,
                })
            })
            .map_err(|e| format!("执行对局历史查询失败: {}", e))?;

        let mut list = Vec::new();
        for row in rows {
            list.push(row.map_err(|e| format!("解析对局历史行失败: {}", e))?);
        }

        Ok(list)
    }

    /// 切换或更新玩家置顶状态
    pub fn update_pin(&self, puuid: &str, self_puuid: &str, is_pinned: bool) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let pinned_val = if is_pinned { 1 } else { 0 };
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "UPDATE saved_players SET is_pinned = ?1, update_at = ?2 WHERE puuid = ?3 AND (?4 = '' OR self_puuid = ?4)",
            params![pinned_val, now, puuid, self_puuid],
        )
        .map_err(|e| format!("更新置顶失败: {}", e))?;

        Ok(())
    }

    /// 更新玩家标签与备忘录
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

        conn.execute(
            "UPDATE saved_players SET tag = ?1, notes = ?2, update_at = ?3 WHERE puuid = ?4 AND (?5 = '' OR self_puuid = ?5)",
            params![tag, notes, now, puuid, self_puuid],
        )
        .map_err(|e| format!("更新玩家标签与备注失败: {}", e))?;

        Ok(())
    }

    /// 删除玩家记录以及关联的相遇历史
    pub fn delete_player(&self, puuid: &str, self_puuid: &str) -> Result<(), String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| format!("获取数据库锁失败: {}", e))?;

        let tx = conn.transaction().map_err(|e| e.to_string())?;

        if self_puuid.is_empty() {
            tx.execute("DELETE FROM saved_players WHERE puuid = ?1", params![puuid])
                .map_err(|e| e.to_string())?;
            tx.execute(
                "DELETE FROM encountered_games WHERE puuid = ?1",
                params![puuid],
            )
            .map_err(|e| e.to_string())?;
        } else {
            tx.execute(
                "DELETE FROM saved_players WHERE puuid = ?1 AND self_puuid = ?2",
                params![puuid, self_puuid],
            )
            .map_err(|e| e.to_string())?;
            tx.execute(
                "DELETE FROM encountered_games WHERE puuid = ?1 AND self_puuid = ?2",
                params![puuid, self_puuid],
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 批量记录对局相遇玩家（对局结束时调用）
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

            tx.execute(
                "INSERT INTO encountered_games (game_id, puuid, self_puuid, region, rso_platform_id, queue_type, update_at, champion_id)
                 VALUES (?1, ?2, ?3, '', '', ?4, ?5, ?6)",
                params![game_id as i64, player.puuid, self_puuid, queue_type, ts, player.champion_id],
            )
            .map_err(|e| format!("记录相遇对局失败: {}", e))?;

            recorded += 1;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(recorded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDb(PathBuf);
    impl TempDb {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("yuumi_test_{}_{}.db", name, chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
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
    }
}
