//! 玩家身份兼容性与槽位继承判定（下沉自前端 src/utils/playerIdentity.ts）

/// 新号判定等级上限：空战绩且等级在此之下视为从未打过的新号，不标隐藏
pub const NEW_PLAYER_MAX_LEVEL: u32 = 30;

/// 身份判定需要的输入载荷
#[derive(Debug, Clone, Default)]
pub struct IdentityPayload<'a> {
    pub puuid: Option<&'a str>,
    pub summoner_id: Option<i64>,
    pub cell_id: Option<i64>,
}

/// 已存在的条目身份载荷
#[derive(Debug, Clone, Default)]
pub struct EntryIdentity<'a> {
    pub puuid: Option<&'a str>,
    pub summoner_id: Option<i64>,
}

/// 槽位英雄 ID 继承：保留已有英雄 ID，只有条目与 incoming 身份明确冲突时才严禁串用
pub fn inherit_placeholder_champion(
    current_champion_id: i64,
    cell_id: i64,
    entry_identity: Option<EntryIdentity>,
    incoming: Option<IdentityPayload>,
) -> i64 {
    if current_champion_id <= 0 {
        return 0;
    }
    if let (Some(entry), Some(inc)) = (entry_identity, incoming) {
        let in_puuid = inc.puuid.unwrap_or("").trim();
        let in_sid = inc.summoner_id.unwrap_or(0);
        let in_sid_real = in_sid != 0 && in_sid != cell_id;

        let e_puuid = entry.puuid.unwrap_or("").trim();
        let e_raw_sid = entry.summoner_id.unwrap_or(0);
        let e_sid = if e_raw_sid == cell_id { 0 } else { e_raw_sid };

        // 仅在明确与不同真实身份冲突时拒绝继承
        if !in_puuid.is_empty() && !e_puuid.is_empty() && in_puuid != e_puuid {
            return 0;
        }
        if in_sid_real && e_sid != 0 && in_sid != e_sid {
            return 0;
        }
    }
    current_champion_id
}

/// teamParticipantId / partyId 有效值提取：0、"0"、"" 等空值视为无效
pub fn get_valid_tpid(team_participant_id: Option<&str>, party_id: Option<&str>) -> Option<String> {
    if let Some(tpid) = team_participant_id {
        let trimmed = tpid.trim();
        if !trimmed.is_empty() && trimmed != "0" {
            return Some(trimmed.to_string());
        }
    }
    if let Some(pid) = party_id {
        let trimmed = pid.trim();
        if !trimmed.is_empty() && trimmed != "0" {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// 身份门禁（canonical）：incoming 的真实身份键与条目是否冲突。
/// puuid 非空即真实；summonerId 仅在非 0 且不等于 cell 槽位时视为真实
/// （选人/对局切换时 cellId 会被充作 sid 兜底，不可参与比对）。
/// 严防脱敏匿名玩家复用已有实名玩家的数据：
/// 任何阶段无实名身份槽位严禁继承有实名身份旧数据，反之亦然！
pub fn is_identity_compatible(entry: Option<EntryIdentity>, incoming: IdentityPayload) -> bool {
    let entry = match entry {
        Some(e) => e,
        None => return false,
    };

    let cell = incoming.cell_id.unwrap_or(-1);
    let in_puuid = incoming.puuid.unwrap_or("").trim();
    let in_sid = incoming.summoner_id.unwrap_or(0);
    let in_sid_real = in_sid != 0 && in_sid != cell;

    let e_puuid = entry.puuid.unwrap_or("").trim();
    let e_raw_sid = entry.summoner_id.unwrap_or(0);
    let e_sid = if e_raw_sid == cell { 0 } else { e_raw_sid };

    if !in_puuid.is_empty() && !e_puuid.is_empty() && in_puuid != e_puuid {
        return false;
    }
    if in_sid_real && e_sid != 0 && in_sid != e_sid {
        return false;
    }

    let incoming_has_identity = !in_puuid.is_empty() || in_sid_real;
    let entry_has_identity = !e_puuid.is_empty() || e_sid != 0;

    if incoming_has_identity != entry_has_identity {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_compatibility_empty() {
        assert!(!is_identity_compatible(None, IdentityPayload::default()));
    }

    #[test]
    fn test_identity_compatibility_anonymous_vs_real() {
        let real_entry = EntryIdentity {
            puuid: Some("puuid-123"),
            summoner_id: Some(1001),
        };
        let anon_incoming = IdentityPayload {
            cell_id: Some(0),
            summoner_id: Some(0),
            puuid: None,
        };
        assert!(!is_identity_compatible(Some(real_entry), anon_incoming));
    }

    #[test]
    fn test_identity_compatibility_same_real() {
        let real_entry = EntryIdentity {
            puuid: Some("puuid-123"),
            summoner_id: Some(1001),
        };
        let same_incoming = IdentityPayload {
            cell_id: Some(0),
            summoner_id: Some(1001),
            puuid: Some("puuid-123"),
        };
        assert!(is_identity_compatible(Some(real_entry), same_incoming));
    }

    #[test]
    fn test_identity_compatibility_conflicting_puuid() {
        let real_entry = EntryIdentity {
            puuid: Some("puuid-123"),
            summoner_id: Some(1001),
        };
        let diff_incoming = IdentityPayload {
            cell_id: Some(0),
            summoner_id: Some(1001),
            puuid: Some("puuid-456"),
        };
        assert!(!is_identity_compatible(Some(real_entry), diff_incoming));
    }

    #[test]
    fn test_valid_tpid() {
        assert_eq!(get_valid_tpid(Some("0"), Some("123")), Some("123".to_string()));
        assert_eq!(get_valid_tpid(Some("party-1"), Some("123")), Some("party-1".to_string()));
        assert_eq!(get_valid_tpid(Some("0"), Some("0")), None);
        assert_eq!(get_valid_tpid(None, None), None);
    }
}
