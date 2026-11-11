//! LCU WebSocket 事件的投影层。
//!
//! LCU 推送的 session 载荷包含大量前端从不读取的字段（obfuscatedPuuid、obfuscatedSummonerId、
//! selectedSkinId、wardSkinId、summonerIconId、team、entitledFeatureState 等）。
//! 投影规则：字段白名单严格对齐前端消费契约。

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

/// gameflow 对局内英雄选择记录（`PlayerChampionSelection`），
/// 用于 teamOne/teamTwo 成员缺少 championId 时按 puuid / summonerId 回填英雄。
const PLAYER_CHAMPION_SELECTION_KEYS: &[&str] =
    &["summonerInternalName", "puuid", "summonerId", "championId"];

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

/// 对数组的每个元素执行 `pick`；源不是数组时返回 `None`。
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

    for key in [
        "localPlayerCellId",
        "benchEnabled",
        "queueId",
        "isCustomGame",
        "rerollsRemaining",
    ] {
        if let Some(value) = obj.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }

    Value::Object(out)
}

fn project_gameflow_session(data: &Value) -> Value {
    let Some(game_data) = data.get("gameData").filter(|v| v.is_object()) else {
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

    if let Some(value) = pick_all(
        game_data.get("playerChampionSelections"),
        PLAYER_CHAMPION_SELECTION_KEYS,
    ) {
        projected.insert("playerChampionSelections".to_string(), value);
    }

    if let Some(value) = game_data.get("gameId") {
        projected.insert("gameId".to_string(), value.clone());
    }

    let mut out = Map::new();
    out.insert("gameData".to_string(), Value::Object(projected));
    Value::Object(out)
}

pub fn project(key: &str, data: &Value) -> Value {
    match key {
        "/lol-champ-select/v1/session" => project_champ_select_session(data),
        "/lol-gameflow/v1/session" => project_gameflow_session(data),
        _ => data.clone(),
    }
}

#[derive(Debug, Default)]
pub struct EventDedup {
    last: HashMap<&'static str, String>,
}

impl EventDedup {
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
            "rerollsRemaining": 2,
            "chatDetails": { "mucJwtDto": { "token": "secret" } }
        });

        let out = project("/lol-champ-select/v1/session", &data);
        let obj = out.as_object().unwrap();

        assert!(!obj.contains_key("chatDetails"));

        let player = &obj["myTeam"][0];
        assert_eq!(player["championId"], json!(103));
        assert_eq!(player["assignedPosition"], json!("middle"));
        assert_eq!(player["teamParticipantId"], json!("t1"));
        assert!(player.get("obfuscatedPuuid").is_none());
        assert!(player.get("selectedSkinId").is_none());

        assert!(obj["bans"].get("numBans").is_none());
        assert_eq!(obj["bans"]["myTeamBans"], json!([1]));
        assert!(obj["pickOrderSwaps"][0].get("extra").is_none());
        assert_eq!(obj["pickOrderSwaps"][0]["state"], json!("AVAILABLE"));
        assert!(obj["benchChampions"][0].get("extra").is_none());
        assert!(obj["timer"].get("internalNowInEpochMs").is_none());
        assert_eq!(obj["timer"]["adjustedTimeLeftInPhase"], json!(27000));

        assert_eq!(obj["localPlayerCellId"], json!(2));
        assert_eq!(obj["benchEnabled"], json!(true));
        assert_eq!(obj["queueId"], json!(2400));
        assert_eq!(obj["isCustomGame"], json!(false));
        assert_eq!(obj["rerollsRemaining"], json!(2));
    }

    #[test]
    fn champ_select_preserves_missing_sections() {
        let data = json!({ "localPlayerCellId": 0, "timer": { "phase": "PLANNING" } });
        let out = project("/lol-champ-select/v1/session", &data);
        let obj = out.as_object().unwrap();

        assert!(!obj.contains_key("myTeam"));
        assert!(!obj.contains_key("actions"));
        assert_eq!(obj["timer"]["phase"], json!("PLANNING"));
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
                    "obfuscatedSummonerId": 1,
                    "selectedSkinId": 1000
                }],
                "teamTwo": [],
                "playerChampionSelections": [{
                    "summonerInternalName": "player-1",
                    "puuid": "p-1",
                    "summonerId": 1,
                    "championId": 266,
                    "obfuscatedPuuid": "xxx",
                    "spell1Id": 4
                }],
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

        let sel = &gd["playerChampionSelections"][0];
        assert_eq!(sel["puuid"], json!("p-1"));
        assert_eq!(sel["championId"], json!(266));
        assert_eq!(sel["summonerId"], json!(1));
        assert_eq!(sel["summonerInternalName"], json!("player-1"));
        assert!(sel.get("obfuscatedPuuid").is_none());
        assert!(sel.get("spell1Id").is_none());

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

        let changed = json!({ "localPlayerCellId": 2 });
        assert!(dedup.accept("/lol-champ-select/v1/session", &changed));
    }

    #[test]
    fn dedup_isolates_uris() {
        let mut dedup = EventDedup::default();
        let payload = json!("ChampSelect");

        assert!(dedup.accept("/lol-gameflow/v1/gameflow-phase", &payload));
        assert!(dedup.accept("/lol-matchmaking/v1/ready-check", &payload));
    }
}
