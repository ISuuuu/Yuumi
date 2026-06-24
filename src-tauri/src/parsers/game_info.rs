use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{build_auth_header, AppState};
use crate::parsers::match_parser::{LcuMatchHistoryResponse, MatchDisplay};

// ─── 输入数据结构（来自 champ select 的 myTeam / theirTeam）───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePlayerInfo {
    pub summoner_id: u64,
    #[serde(default)]
    pub champion_id: i32,
}

// ─── 输出数据结构 ───

/// 单个玩家的战绩汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerGameSummary {
    pub summoner_id: u64,
    pub puuid: String,
    pub name: String,
    pub level: u32,
    pub champion_id: i32,
    pub champion_icon_url: String,
    pub rank_info: Option<RankInfo>,
    pub recent_kda: (i32, i32, i32), // (kills, deaths, assists)
    pub recent_win_rate: f64,
    pub recent_games: Vec<MatchDisplay>,
    pub fate_flag: Option<String>, // "ally" | "enemy" | null
    pub recently_champion_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankInfo {
    pub tier: String,
    pub rank: String,
    pub league_points: i32,
    pub wins: i32,
    pub losses: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcuSummonerById {
    pub puuid: Option<String>,
    pub display_name: Option<String>,
    pub summoner_level: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct _LcuRankedEntry {
    pub tier: Option<String>,
    pub rank: Option<String>,
    pub league_points: Option<i32>,
    pub wins: Option<i32>,
    pub losses: Option<i32>,
    pub queue_type: Option<String>,
}

// ─── 十人并发查询 ───

/// 并发查询所有玩家的战绩、段位、KDA，并判定上局关系
#[tauri::command]
pub async fn get_game_player_summaries(
    players: Vec<GamePlayerInfo>,
    current_summoner_id: u64,
    app_state: State<'_, AppState>,
) -> Result<Vec<PlayerGameSummary>, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let assets = app_state.game_data.read().await.clone();

    // 并发查询所有玩家
    let mut handles = Vec::new();
    for player in &players {
        let auth = auth.clone();
        let base = base.clone();
        let http = lcu.http_client.clone();
        let player = player.clone();
        let assets = assets.clone();

        handles.push(tokio::spawn(async move {
            fetch_player_summary(&http, &base, &auth, &player, current_summoner_id, &assets).await
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Some(summary)) => results.push(summary),
            Ok(None) => {}
            Err(e) => log::error!("玩家查询任务 panic: {}", e),
        }
    }

    Ok(results)
}

async fn fetch_player_summary(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    player: &GamePlayerInfo,
    current_summoner_id: u64,
    assets: &crate::lcu::game_data::GameDataAssets,
) -> Option<PlayerGameSummary> {
    // 1. 获取召唤师信息
    let summoner_url = format!(
        "{}/lol-summoner/v1/summoners/{}",
        base, player.summoner_id
    );
    let summoner: LcuSummonerById = http
        .get(&summoner_url)
        .header("Authorization", auth)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let puuid = summoner.puuid?;
    let name = summoner.display_name.unwrap_or_default();
    let level = summoner.summoner_level.unwrap_or(0);

    // 2. 获取段位信息
    let rank_url = format!(
        "{}/lol-ranked/v1/ranked-stats/{}",
        base, puuid
    );
    let rank_info = match http.get(&rank_url).header("Authorization", auth).send().await {
        Ok(resp) => resp.json::<serde_json::Value>().await.ok()
            .and_then(|v| parse_rank_from_value(&v)),
        Err(_) => None,
    };

    // 3. 获取战绩
    let games_url = format!(
        "{}/lol-match-history/v1/products/lol/{}/matches?begIndex=0&endIndex=11",
        base, puuid
    );
    let games_info = match http.get(&games_url).header("Authorization", auth).send().await {
        Ok(resp) => resp.json::<LcuMatchHistoryResponse>().await.ok(),
        Err(_) => None,
    };

    let (recent_games, total_kills, total_deaths, total_assists, wins) = match games_info {
        Some(history) => {
            let games: Vec<MatchDisplay> = history
                .games
                .games
                .iter()
                .filter(|g| !g.participants.is_empty())
                .map(|g| g.to_display(assets))
                .collect();

            let (k, d, a, w) = games.iter().fold((0, 0, 0, 0), |(k, d, a, w), g| {
                (
                    k + g.kills,
                    d + g.deaths,
                    a + g.assists,
                    w + if g.win { 1 } else { 0 },
                )
            });

            (games, k, d, a, w)
        }
        None => (Vec::new(), 0, 0, 0, 0),
    };

    let game_count = (recent_games.len()).max(1) as f64;
    let win_rate = wins as f64 / game_count;

    // 4. 上局宿命判定：检查最近一局是否与当前玩家有交集
    let fate_flag = if let Some(first_game) = recent_games.first() {
        check_fate(http, base, auth, first_game.game_id, current_summoner_id, &puuid).await
    } else {
        None
    };

    // 5. 最近常用英雄
    let recently_champion_id = recent_games.first().map(|g| g.champion_id);

    let champion_icon_url = format!(
        "/lol-game-data/assets/v1/champion-icons/{}.png",
        player.champion_id
    );

    Some(PlayerGameSummary {
        summoner_id: player.summoner_id,
        puuid,
        name,
        level,
        champion_id: player.champion_id,
        champion_icon_url,
        rank_info,
        recent_kda: (total_kills, total_deaths, total_assists),
        recent_win_rate: win_rate,
        recent_games,
        fate_flag,
        recently_champion_id,
    })
}

/// 检查上局关系：当前玩家是上局的队友还是对手
async fn check_fate(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    game_id: u64,
    current_summoner_id: u64,
    _target_puuid: &str,
) -> Option<String> {
    let url = format!(
        "{}/lol-match-history/v1/games/{}",
        base, game_id
    );
    let resp = http
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .ok()?;
    let detail: serde_json::Value = resp.json().await.ok()?;

    let participants = detail.get("participants")?.as_array()?;
    let identities = detail.get("participantIdentities")?.as_array()?;

    // 找到当前玩家和目标玩家所在的队伍
    let mut current_team_id: Option<i32> = None;
    let _target_team_id: Option<i32> = None;

    // 通过 participantIdentities 匹配 summonerId → participantId → teamId
    for ident in identities {
        let player_data = ident.get("player")?;
        let summoner_id = player_data.get("summonerId").and_then(|v| v.as_u64())?;
        let participant_id = ident.get("participantId").and_then(|v| v.as_i64())? as i32;

        // 在 participants 中找到对应的 teamId
        for p in participants {
            let pid = p.get("participantId").and_then(|v| v.as_i64())? as i32;
            if pid == participant_id {
                let team_id = p.get("teamId").and_then(|v| v.as_i64())? as i32;
                if summoner_id == current_summoner_id {
                    current_team_id = Some(team_id);
                }
            }
        }
    }

    // 如果当前玩家不在这局中，无法判定
    if current_team_id.is_none() {
        return None;
    }

    // 这里简化处理：如果有上局数据，标记为"有缘"
    // 完整实现需要对比目标玩家的 teamId
    Some("encountered".to_string())
}

fn parse_rank_from_value(v: &serde_json::Value) -> Option<RankInfo> {
    // 查找 RANKED_SOLO_5x5 队列
    let queues = v.get("queues")?.as_array()?;
    for queue in queues {
        let queue_type = queue.get("queueType").and_then(|q| q.as_str()).unwrap_or("");
        if queue_type == "RANKED_SOLO_5x5" {
            return Some(RankInfo {
                tier: queue
                    .get("tier")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
                rank: queue
                    .get("rank")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string(),
                league_points: queue.get("leaguePoints").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                wins: queue.get("wins").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                losses: queue.get("losses").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            });
        }
    }
    None
}
