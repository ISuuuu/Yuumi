//! LCU WebSocket 事件的 IPC 投影层。
//!
//! LCU 推送的 session 载荷包含大量前端从不读取的字段（obfuscatedPuuid、obfuscatedSummonerId、
//! selectedSkinId、wardSkinId、summonerIconId、team、entitledFeatureState 等），
//! 而本项目通过 Tauri IPC 将事件整体序列化后跨进程传输到 WebView。
//! 选人阶段的 session 每秒最多推送 20 次（见 `ws.rs` 的 `SESSION_THROTTLE_MS`），
//! 载荷每膨胀 1KB，每秒就要多搬运 20KB，且 WebView 侧还需完整 JSON 解析一遍。
//!
//! 投影规则：字段白名单严格对齐前端消费契约，即
//! - `src/store/lcuStore.ts` 的 `ChampSelectSession` / `ChampSelectAction` / `ChampSelectPlayer` /
//!   `BenchChampion` 等接口
//! - `src/types/lcu.ts` 的 `GameflowSession` / `GameflowGameData` / `GameflowParticipant`
//! - `src/types/gameInfo.ts` 的 `ChampSelectSessionLike`
//!
//! `vue-tsc --noEmit` 会校验全部消费点，因此这些接口即为可靠的字段白名单。
//! **前端新增消费字段时必须同步扩展本模块，否则该字段会被静默丢弃。**

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// 选人会话中单个玩家（`ChampSelectPlayer`）。
const CHAMP_SELECT_PLAYER_KEYS: &[&str] = &[
    "cellId",
    "championId",
    "championPickIntent",
    "assignedPosition",
    "summonerId",
    "puuid",
    "displayName",
    "teamParticipantId",
    "partyId",
    "isHumanoid",
    "bot",
    "isBot",
];

/// 选人会话中的单个动作（`ChampSelectAction`）。
const CHAMP_SELECT_ACTION_KEYS: &[&str] = &[
    "actorCellId",
    "championId",
    "completed",
    "id",
    "isInProgress",
    "type",
];

/// 选人会话倒计时（`ChampSelectTimer`）。
const CHAMP_SELECT_TIMER_KEYS: &[&str] = &["adjustedTimeLeftInPhase", "phase", "totalTimeInPhase"];

/// 板凳席英雄（`BenchChampion`）。
const BENCH_CHAMPION_KEYS: &[&str] = &["championId", "isMine"];

/// 英雄交换 / 交易（`ChampSelectSwap` / `ChampSelectTrade`）。
const SWAP_TRADE_KEYS: &[&str] = &["id", "state"];

/// gameflow 队列信息（`GameflowQueue`）。
const GAMEFLOW_QUEUE_KEYS: &[&str] = &["id", "gameMode", "isCustom"];

/// gameflow 对局参与者（`GameflowParticipant`）。
const GAMEFLOW_PARTICIPANT_KEYS: &[&str] = &[
    "summonerId",
    "puuid",
    "displayName",
    "summonerName",
    "gameName",
    "tagLine",
    "profileIconId",
    "championId",
    "cellId",
    "teamParticipantId",
    "partyId",
    "assignedPosition",
    "bot",
    "isBot",
    "botChampionId",
    "botName",
];

/// 从对象中按白名单拷贝字段；源不是对象时返回空对象。
/// 保留字段原值不做类型转换，避免改变前端既有判定语义。
fn pick(src: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    if let Some(obj) = src.as_object() {
        for key in keys {
            if let Some(value) = obj.get(*key) {
                out.insert((*key).to_string(), value.clone());
            }
        }
    }
    Value::Object(out)
}

/// 对数组的每个元素执行 `pick`；源不是数组时返回 `None`（保持"字段缺失"语义）。
fn pick_all(src: Option<&Value>, keys: &[&str]) -> Option<Value> {
    let list = src?.as_array()?;
    Some(Value::Array(
        list.iter().map(|item| pick(item, keys)).collect(),
    ))
}

fn project_champ_select_session(data: &Value) -> Value {
    let Some(obj) = data.as_object() else {
        return Value::Null;
    };

    let mut out = Map::new();

    // actions：二维数组（每个回合一组动作）
    if let Some(groups) = obj.get("actions").and_then(|v| v.as_array()) {
        let projected: Vec<Value> = groups
            .iter()
            .map(|group| {
                Value::Array(
                    group
                        .as_array()
                        .map(|acts| {
                            acts.iter()
                                .map(|a| pick(a, CHAMP_SELECT_ACTION_KEYS))
                                .collect()
                        })
                        .unwrap_or_default(),
                )
            })
            .collect();
        out.insert("actions".to_string(), Value::Array(projected));
    }

    for key in ["myTeam", "theirTeam"] {
        if let Some(value) = pick_all(obj.get(key), CHAMP_SELECT_PLAYER_KEYS) {
            out.insert(key.to_string(), value);
        }
    }

    for key in ["pickOrderSwaps", "trades"] {
        if let Some(value) = pick_all(obj.get(key), SWAP_TRADE_KEYS) {
            out.insert(key.to_string(), value);
        }
    }

    if let Some(value) = pick_all(obj.get("benchChampions"), BENCH_CHAMPION_KEYS) {
        out.insert("benchChampions".to_string(), value);
    }

    if let Some(bans) = obj.get("bans") {
        out.insert(
            "bans".to_string(),
            pick(bans, &["myTeamBans", "theirTeamBans"]),
        );
    }

    if let Some(timer) = obj.get("timer") {
        out.insert("timer".to_string(), pick(timer, CHAMP_SELECT_TIMER_KEYS));
    }

    // 标量字段直接透传
    for key in [
        "localPlayerCellId",
        "benchEnabled",
        "queueId",
        "isCustomGame",
    ] {
        if let Some(value) = obj.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }

    Value::Object(out)
}

fn project_gameflow_session(data: &Value) -> Value {
    let Some(game_data) = data.get("gameData").filter(|v| v.is_object()) else {
        // 源载荷没有 gameData 时保持 null，前端依赖该判定提前退出
        return Value::Null;
    };

    let mut projected = Map::new();

    if let Some(queue) = game_data.get("queue") {
        projected.insert("queue".to_string(), pick(queue, GAMEFLOW_QUEUE_KEYS));
    }

    for key in ["teamOne", "teamTwo"] {
        if let Some(value) = pick_all(game_data.get(key), GAMEFLOW_PARTICIPANT_KEYS) {
            projected.insert(key.to_string(), value);
        }
    }

    if let Some(value) = game_data.get("gameId") {
        projected.insert("gameId".to_string(), value.clone());
    }

    let mut out = Map::new();
    out.insert("gameData".to_string(), Value::Object(projected));
    Value::Object(out)
}

/// 按事件 URI 前缀生成前端投影载荷。
///
/// 只有载荷较大且字段冗余的两个 session 事件做投影；其余事件（gameflow-phase 裸字符串、
/// current-champion 裸数字、ready-check 仅 5 个字段）原样透传，避免无收益的中间拷贝。
pub fn project(key: &str, data: &Value) -> Value {
    match key {
        "/lol-champ-select/v1/session" => project_champ_select_session(data),
        "/lol-gameflow/v1/session" => project_gameflow_session(data),
        _ => data.clone(),
    }
}

/// 发送前去重：同一 URI 的投影结果与上次完全相同时跳过下发。
///
/// 选中人会话在无人操作时仍会因倒计时变化而持续推送，但 gameflow session、
/// ready-check 等事件在状态不变时会被 LCU 重复推送，去重可省掉这部分跨进程传输与前端解析。
/// 实例按 WS 连接创建，重连后自动清空，避免用旧快照吞掉重连后的首个事件。
#[derive(Debug, Default)]
pub struct EventDedup {
    last: HashMap<&'static str, String>,
}

impl EventDedup {
    /// 返回 true 表示内容已变化、需要下发；false 表示与上次一致可跳过。
    /// 序列化失败（理论上不会发生）时选择下发，保证不丢事件。
    pub fn accept(&mut self, key: &'static str, projected: &Value) -> bool {
        let Ok(encoded) = serde_json::to_string(projected) else {
            return true;
        };
        match self.last.get(key) {
            Some(previous) if previous == &encoded => false,
            _ => {
                self.last.insert(key, encoded);
                true
            }
        }
    }
}

/// 下发给前端的 LCU WebSocket 事件载荷。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsEventPayload<'a> {
    pub uri: &'a str,
    pub event_type: &'a str,
    pub data: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn champ_select_drops_unconsumed_fields() {
        let data = json!({
            "actions": [],
            "localPlayerCellId": 2,
            "benchEnabled": true,
            "myTeam": [{
                "cellId": 2,
                "championId": 103,
                "championPickIntent": 0,
                "assignedPosition": "middle",
                "puuid": "p-2",
                "summonerId": 42,
                "displayName": "玩家二",
                "teamParticipantId": "t1",
                "partyId": "t1",
                "isHumanoid": false,
                // 以下字段前端不消费，应被丢弃
                "obfuscatedPuuid": "xxx",
                "obfuscatedSummonerId": 999,
                "selectedSkinId": 103000,
                "wardSkinId": -1,
                "summonerIconId": 4568,
                "spell1Id": 4,
                "spell2Id": 12,
                "team": 1
            }],
            "theirTeam": [],
            "bans": { "myTeamBans": [1], "theirTeamBans": [2], "numBans": 10 },
            "pickOrderSwaps": [{ "id": 1, "state": "AVAILABLE", "extra": 1 }],
            "trades": [],
            "benchChampions": [{ "championId": 22, "isMine": true, "extra": 1 }],
            "timer": {
                "adjustedTimeLeftInPhase": 27000,
                "phase": "BAN_PICK",
                "totalTimeInPhase": 30000,
                "internalNowInEpochMs": 1700000000000u64,
                "isInfinite": false
            },
            "queueId": 2400,
            "isCustomGame": false,
            "chatDetails": { "mucJwtDto": { "token": "secret" } }
        });

        let out = project("/lol-champ-select/v1/session", &data);
        let obj = out.as_object().unwrap();

        // 未消费的顶层字段被丢弃
        assert!(!obj.contains_key("chatDetails"));

        let player = &obj["myTeam"][0];
        assert_eq!(player["championId"], json!(103));
        assert_eq!(player["assignedPosition"], json!("middle"));
        assert_eq!(player["teamParticipantId"], json!("t1"));
        assert!(player.get("obfuscatedPuuid").is_none());
        assert!(player.get("selectedSkinId").is_none());

        // 嵌套结构同样只保留白名单
        assert!(obj["bans"].get("numBans").is_none());
        assert_eq!(obj["bans"]["myTeamBans"], json!([1]));
        assert!(obj["pickOrderSwaps"][0].get("extra").is_none());
        assert_eq!(obj["pickOrderSwaps"][0]["state"], json!("AVAILABLE"));
        assert!(obj["benchChampions"][0].get("extra").is_none());
        assert!(obj["timer"].get("internalNowInEpochMs").is_none());
        assert_eq!(obj["timer"]["adjustedTimeLeftInPhase"], json!(27000));

        // 标量字段保留
        assert_eq!(obj["localPlayerCellId"], json!(2));
        assert_eq!(obj["benchEnabled"], json!(true));
        assert_eq!(obj["queueId"], json!(2400));
        assert_eq!(obj["isCustomGame"], json!(false));
    }

    #[test]
    fn champ_select_preserves_missing_sections() {
        // 选人早期 LCU 只推部分字段，缺失的段不应被补成空值
        let data = json!({ "localPlayerCellId": 0, "timer": { "phase": "PLANNING" } });
        let out = project("/lol-champ-select/v1/session", &data);
        let obj = out.as_object().unwrap();

        assert!(!obj.contains_key("myTeam"));
        assert!(!obj.contains_key("actions"));
        assert_eq!(obj["timer"]["phase"], json!("PLANNING"));
        // timer 缺失的字段不补 null
        assert!(obj["timer"].get("adjustedTimeLeftInPhase").is_none());
    }

    #[test]
    fn champ_select_null_stays_null() {
        assert_eq!(
            project("/lol-champ-select/v1/session", &Value::Null),
            Value::Null
        );
    }

    #[test]
    fn gameflow_session_keeps_only_consumed_paths() {
        let data = json!({
            "gameData": {
                "gameId": 7000000001i64,
                "queue": { "id": 420, "gameMode": "CLASSIC", "isCustom": false, "type": "x" },
                "teamOne": [{
                    "puuid": "p-1",
                    "summonerId": 1,
                    "championId": 1,
                    "cellId": 0,
                    "assignedPosition": "top",
                    "teamParticipantId": "a",
                    // 未消费
                    "obfuscatedSummonerId": 1,
                    "selectedSkinId": 1000
                }],
                "teamTwo": [],
                // 未消费
                "gameServerAddress": "1.2.3.4",
                "isSpectating": false
            },
            "gameClient": { "observerServerIp": "1.2.3.4" },
            "map": { "id": 11 }
        });

        let out = project("/lol-gameflow/v1/session", &data);
        let root = out.as_object().unwrap();
        assert!(!root.contains_key("gameClient"));
        assert!(!root.contains_key("map"));

        let gd = root["gameData"].as_object().unwrap();
        assert!(!gd.contains_key("gameServerAddress"));
        assert!(!gd.contains_key("isSpectating"));
        assert_eq!(gd["gameId"], json!(7000000001i64));
        assert_eq!(gd["queue"]["id"], json!(420));
        assert_eq!(gd["queue"]["gameMode"], json!("CLASSIC"));
        assert!(gd["queue"].get("type").is_none());

        let p = &gd["teamOne"][0];
        assert_eq!(p["puuid"], json!("p-1"));
        assert_eq!(p["teamParticipantId"], json!("a"));
        assert!(p.get("obfuscatedSummonerId").is_none());
    }

    #[test]
    fn gameflow_session_without_game_data_is_null() {
        assert_eq!(
            project("/lol-gameflow/v1/session", &json!({ "gameClient": {} })),
            Value::Null
        );
    }

    #[test]
    fn other_events_pass_through_untouched() {
        let data = json!({ "playerResponse": "Accepted", "state": "InProgress", "timer": 8 });
        assert_eq!(project("/lol-matchmaking/v1/ready-check", &data), data);
        assert_eq!(
            project("/lol-gameflow/v1/gameflow-phase", &json!("ChampSelect")),
            json!("ChampSelect")
        );
    }

    #[test]
    fn dedup_rejects_identical_payloads() {
        let mut dedup = EventDedup::default();
        let payload = json!({ "localPlayerCellId": 1 });

        assert!(dedup.accept("/lol-champ-select/v1/session", &payload));
        assert!(!dedup.accept("/lol-champ-select/v1/session", &payload));

        // 内容变化后重新下发
        let changed = json!({ "localPlayerCellId": 2 });
        assert!(dedup.accept("/lol-champ-select/v1/session", &changed));
    }

    #[test]
    fn dedup_isolates_uris() {
        let mut dedup = EventDedup::default();
        let payload = json!("ChampSelect");

        assert!(dedup.accept("/lol-gameflow/v1/gameflow-phase", &payload));
        // 不同 URI 相同内容不应互相抑制
        assert!(dedup.accept("/lol-matchmaking/v1/ready-check", &payload));
    }

    /// 构造一份贴近 LCU 真实形态的满编选人会话（10 人 + 65 个板凳席英雄）。
    /// 其中包含 playerType、entitledFeatureType、selectedSkinId、spell1Id/2Id、
    /// nameVisibilityType、obfuscated* 等前端从不消费的字段。
    fn realistic_champ_select_session() -> Value {
        fn player(cell: i64, pos: &str) -> Value {
            json!({
                "assignedPosition": pos,
                "cellId": cell,
                "championId": 0,
                "championPickIntent": 103 + cell,
                "entitledFeatureType": "",
                "playerType": "PLAYER",
                "puuid": format!("{:08x}-1111-2222-3333-{:012x}", cell, cell * 7919),
                "selectedSkinId": 103000 + cell,
                "spell1Id": 4,
                "spell2Id": 14,
                "summonerId": 1234567890123i64 + cell,
                "team": 1,
                "wardSkinId": -1,
                "obfuscatedPuuid": "A".repeat(88),
                "obfuscatedSummonerId": 9000000 + cell,
                "displayName": format!("Player{}", cell),
                "nameVisibilityType": "VISIBLE",
                "isHumanoid": false,
                "bot": false,
                "isBot": false,
                "teamParticipantId": format!("00000000-0000-4000-8000-{:012x}", cell),
                "partyId": format!("00000000-0000-4000-8000-{:012x}", cell / 2)
            })
        }

        let mut my_team = Vec::new();
        let mut their_team = Vec::new();
        for cell in 0..5 {
            my_team.push(player(cell, "middle"));
            their_team.push(player(cell + 5, "top"));
        }

        let actions: Vec<Value> = (0..5)
            .map(|turn| {
                let group: Vec<Value> = (0..2)
                    .map(|slot| {
                        json!({
                            "actorCellId": panel_cell(turn, slot),
                            "championId": 0,
                            "completed": false,
                            "id": 10 + turn * 2 + slot,
                            "isAllyAction": true,
                            "isInProgress": turn == 0 && slot == 0,
                            "pickTurn": turn,
                            "type": "pick",
                            "wasExecuted": false
                        })
                    })
                    .collect();
                Value::Array(group)
            })
            .collect();

        let bench: Vec<Value> = (0..65u32)
            .map(|i| json!({ "championId": 1 + i, "isMine": i.is_multiple_of(3) }))
            .collect();

        json!({
            "actions": actions,
            "localPlayerCellId": 0,
            "myTeam": my_team,
            "theirTeam": their_team,
            "bans": { "myTeamBans": [1, 2, 3], "theirTeamBans": [4, 5], "numBans": 10 },
            "pickOrderSwaps": [{ "id": 1, "state": "AVAILABLE" }],
            "trades": [{ "id": 2, "state": "AVAILABLE" }],
            "timer": {
                "adjustedTimeLeftInPhase": 27000,
                "internalNowInEpochMs": 1700000000000u64,
                "isInfinite": false,
                "phase": "BAN_PICK",
                "totalTimeInPhase": 30000
            },
            "benchChampions": bench,
            "benchEnabled": true,
            "queueId": 2400,
            "isCustomGame": false,
            "isSpectating": false,
            "hasSimultaneousBans": false,
            "hasSimultaneousPicks": false,
            "chatDetails": { "chatRoomName": "c1", "mucJwtDto": { "token": "x".repeat(256) } },
            "multiUserChatJwt": "x".repeat(256),
            "recoveryCounter": 0,
            "skipChampionSelect": false
        })
    }

    fn panel_cell(turn: usize, slot: usize) -> i64 {
        turn.is_multiple_of(2) as i64 * 5 + slot as i64
    }

    /// 投影必须显著压缩载荷。后端在选人阶段每秒最多下发 20 次，
    /// 载荷大小直接决定 IPC 与 WebView JSON 解析开销。
    #[test]
    fn projection_meaningfully_shrinks_champ_select_payload() {
        let raw = realistic_champ_select_session();
        let projected = project("/lol-champ-select/v1/session", &raw);

        let raw_len = serde_json::to_string(&raw).unwrap().len();
        let projected_len = serde_json::to_string(&projected).unwrap().len();

        println!(
            "[projection] champ-select session: {raw_len} B → {projected_len} B ({:.1}% 缩减)",
            (1.0 - projected_len as f64 / raw_len as f64) * 100.0
        );

        // 保守门槛：至少压缩 30%。实测约 39%，若低于该值说明白名单被放宽或新增了冗余字段
        assert!(
            projected_len * 10 < raw_len * 7,
            "投影后体积 {projected_len}B 相对原始 {raw_len}B 压缩不足 30%"
        );

        // 投影后仍保留全部约束字段
        assert_eq!(projected["myTeam"].as_array().unwrap().len(), 5);
        assert_eq!(projected["theirTeam"].as_array().unwrap().len(), 5);
        assert_eq!(projected["benchChampions"].as_array().unwrap().len(), 65);
        assert_eq!(projected["actions"].as_array().unwrap().len(), 5);
        assert_eq!(projected["myTeam"][3]["puuid"], raw["myTeam"][3]["puuid"]);
        assert_eq!(projected["timer"]["adjustedTimeLeftInPhase"], json!(27000));
    }
}
