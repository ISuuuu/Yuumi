//! 战利品动作服务层（只读配方查询 + LCU 写操作 + 批量编排）
//!
//! 全部对外函数返回 `Result<T, String>`，统一复用 `LcuClient::get/post`。
//! 本模块只提供纯函数：不启动后台任务、不派发事件，进度以 `Vec<LootActionProgress>` 聚合返回，
//! 由上层（services/live_lcu.rs）在用户主动触发时调用并广播事件。

use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 单次动作允许的最大循环次数（防止异常入参导致死循环）
pub const MAX_LOOP_COUNT: i32 = 500;
/// 单次批量动作允许处理的最大条目数
pub const MAX_BATCH_ITEMS: usize = 500;

/// 把任意次数收敛到 `[0, MAX_LOOP_COUNT]`，所有循环都经此保护
fn clamp_count(n: i32) -> i32 {
    n.clamp(0, MAX_LOOP_COUNT)
}

// ───────────────────────── 数据结构 ─────────────────────────

/// 战利品配方（来自 GET /lol-loot/v1/recipes/initial-item/{lootId}）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LootRecipe {
    /// 配方名，作为 craft 路径参数
    pub recipe_name: String,
    /// 配方槽位（保留原始结构，兼容新版 lootIds / 旧版 slotRules 及精粹材料）
    pub slots: Vec<Value>,
}

/// 可开启战利品（材料卡）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OpenableLoot {
    pub loot_id: String,
    pub name: String,
    pub count: i32,
    pub recipe_name: String,
    pub need_key: bool,
    pub key_loot_id: Option<String>,
    pub key_count: i32,
    pub key_name: Option<String>,
    pub tile_path: Option<String>,
}

/// 智能一键全部开启的单批条目
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OpenBatchItem {
    pub loot_id: String,
    pub name: String,
    pub count: i32,
    pub recipe_name: String,
    pub ingredients: Vec<String>,
}

/// 分解 / 升级请求条目
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DisenchantItem {
    pub loot_id: String,
    pub count: i32,
    /// 升级时前端可传入的精确配方名
    pub upgrade_recipe_name: Option<String>,
}

/// 单条动作进度（current/total/success/loot_name/error_msg）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LootActionProgress {
    pub current: i32,
    pub total: i32,
    pub success: bool,
    pub loot_name: String,
    /// 结果 / 奖励描述
    pub reward_desc: String,
    pub error_msg: Option<String>,
}

/// 战利品写动作种类（服务层统一入口的分派标识）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LootActionKind {
    Open,
    SmartOpenAll,
    Disenchant,
    Reroll,
    Upgrade,
}

/// 统一入口的强类型条目：覆盖 5 类动作各自所需字段（serde default 兜底）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LootActionItem {
    pub loot_id: String,
    pub name: String,
    pub count: i32,
    pub recipe_name: String,
    pub ingredients: Vec<String>,
    pub upgrade_recipe_name: Option<String>,
}

// ───────────────────────── 只读配方查询 ─────────────────────────

/// 拉取指定战利品的配方原始数组（内部使用）
async fn fetch_recipes_raw(client: &LcuClient, loot_id: &str) -> Result<Vec<Value>, String> {
    let path = format!("/lol-loot/v1/recipes/initial-item/{}", loot_id);
    let value = client.get(&path).await?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| "配方数据格式错误，期望数组".to_string())
}

/// 读取指定战利品的配方列表（解析 recipeName / slots）
pub async fn fetch_recipes(client: &LcuClient, loot_id: &str) -> Result<Vec<LootRecipe>, String> {
    let arr = fetch_recipes_raw(client, loot_id).await?;
    Ok(arr
        .iter()
        .map(|r| LootRecipe {
            recipe_name: r
                .get("recipeName")
                .and_then(|v| v.as_str())
                .map_or_else(String::new, |s| s.to_string()),
            slots: r
                .get("slots")
                .and_then(|v| v.as_array())
                .map_or_else(Vec::new, |arr| arr.clone()),
        })
        .collect())
}

/// 在配方列表中查找名称包含关键字（不区分大小写）的配方名
pub async fn find_recipe_name(
    client: &LcuClient,
    loot_id: &str,
    keyword: &str,
) -> Result<String, String> {
    let recipes = fetch_recipes(client, loot_id).await?;
    let kw = keyword.to_ascii_lowercase();
    recipes
        .iter()
        .find_map(|r| {
            let name = r.recipe_name.as_str();
            (!name.is_empty() && name.to_ascii_lowercase().contains(&kw)).then(|| name.to_string())
        })
        .ok_or_else(|| format!("未找到包含关键字 '{}' 的配方", keyword))
}

// ───────────────────────── 原子写操作 ─────────────────────────

/// 执行合成 / 开启 / 分解配方（POST /lol-loot/v1/recipes/{name}/craft?repeat=）
pub async fn craft_loot(
    client: &LcuClient,
    recipe_name: &str,
    ingredients: &[String],
    repeat: i32,
) -> Result<Value, String> {
    // repeat 至少为 1，并加固定上限保护
    let repeat = clamp_count(repeat).max(1);
    let path = format!(
        "/lol-loot/v1/recipes/{}/craft?repeat={}",
        recipe_name, repeat
    );
    let body =
        serde_json::to_value(ingredients).map_err(|e| format!("序列化合成材料失败: {}", e))?;
    client.post(&path, Some(body)).await
}

/// 兑换永久道具（POST /lol-loot/v1/player-loot/{id}/redeem，按 count 逐个调用）
pub async fn redeem_loot(client: &LcuClient, loot_id: &str, count: i32) -> Result<(), String> {
    let path = format!("/lol-loot/v1/player-loot/{}/redeem", loot_id);
    let times = clamp_count(count);
    for _ in 0..times {
        client.post(&path, None).await?;
    }
    Ok(())
}

// ───────────────────────── 进度构造辅助 ─────────────────────────

/// 从 craft 响应里提取奖励描述（added[].playerLoot.itemDesc / lootId）
fn extract_reward(detail: &Value) -> String {
    let Some(added) = detail.get("added").and_then(|v| v.as_array()) else {
        return String::new();
    };
    let names: Vec<String> = added
        .iter()
        .filter_map(|x| {
            let pl = x.get("playerLoot");
            pl.and_then(|p| p.get("itemDesc"))
                .and_then(|d| d.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| pl.and_then(|p| p.get("lootId")).and_then(|d| d.as_str()))
                .map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        names.join(", ")
    }
}

/// 把一次 LCU 调用结果转成一条进度
fn make_progress(
    current: i32,
    total: i32,
    result: &Result<Value, String>,
    loot_name: &str,
) -> LootActionProgress {
    match result {
        Ok(detail) => LootActionProgress {
            current,
            total,
            success: true,
            loot_name: loot_name.to_string(),
            reward_desc: extract_reward(detail),
            error_msg: None,
        },
        Err(e) => LootActionProgress {
            current,
            total,
            success: false,
            loot_name: loot_name.to_string(),
            reward_desc: String::new(),
            error_msg: Some(e.clone()),
        },
    }
}

// ───────────────────────── 批量编排 ─────────────────────────

/// 单个开启 / 合成：对同一配方循环 repeat_count 次 craft（每次 repeat=1）
pub async fn batch_open_loots(
    client: &LcuClient,
    recipe_name: &str,
    ingredients: &[String],
    repeat_count: i32,
) -> Result<Vec<LootActionProgress>, String> {
    let total = clamp_count(repeat_count);
    let mut progress = Vec::new();
    for i in 1..=total {
        let result = craft_loot(client, recipe_name, ingredients, 1).await;
        progress.push(make_progress(i, total, &result, recipe_name));
    }
    Ok(progress)
}

/// 智能一键全部开启：按优先级排序后逐条目按 count 循环 craft（每次 repeat=1）
pub async fn smart_open_all_loots(
    client: &LcuClient,
    batches: &[OpenBatchItem],
) -> Result<Vec<LootActionProgress>, String> {
    let mut sorted: Vec<OpenBatchItem> = batches.iter().take(MAX_BATCH_ITEMS).cloned().collect();
    // 优先级：普通宝箱 > 海克斯宝箱 > 法球 > 胶囊 > 其他
    sorted.sort_by_key(|item| loot_priority(&item.loot_id));

    let total = sorted
        .iter()
        .map(|i| clamp_count(i.count))
        .fold(0i32, |acc, c| acc.saturating_add(c))
        .min(MAX_LOOP_COUNT);

    let mut progress = Vec::new();
    let mut current = 0i32;
    'outer: for item in &sorted {
        let count = clamp_count(item.count);
        for _ in 0..count {
            // 双重上限保护：总进度不得超过 MAX_LOOP_COUNT
            if current >= total {
                break 'outer;
            }
            current += 1;
            let result = craft_loot(client, &item.recipe_name, &item.ingredients, 1).await;
            progress.push(make_progress(current, total, &result, &item.name));
        }
    }
    Ok(progress)
}

/// 开启优先级（数字越小越先开启）
fn loot_priority(loot_id: &str) -> i32 {
    let upper = loot_id.to_ascii_uppercase();
    if upper.contains("CHEST") && !upper.contains("HEXTECH") && !upper.contains("PREMIUM") {
        0
    } else if upper.contains("HEXTECH") {
        1
    } else if upper.contains("ORB") {
        2
    } else if upper.contains("CAPSULE") {
        3
    } else {
        4
    }
}

/// 批量分解：查含 disenchant 关键字的配方，按 count 作为 repeat 一次 craft
pub async fn disenchant_loot(
    client: &LcuClient,
    items: &[DisenchantItem],
) -> Result<Vec<LootActionProgress>, String> {
    let jobs: Vec<&DisenchantItem> = items.iter().take(MAX_BATCH_ITEMS).collect();
    let total = jobs.len() as i32;
    let mut progress = Vec::new();
    for (idx, job) in jobs.iter().enumerate() {
        let current = idx as i32 + 1;
        let result: Result<Value, String> = async {
            let recipe_name = find_recipe_name(client, &job.loot_id, "disenchant").await?;
            let body = vec![job.loot_id.clone()];
            craft_loot(client, &recipe_name, &body, job.count).await
        }
        .await;
        progress.push(make_progress(current, total, &result, &job.loot_id));
    }
    Ok(progress)
}

/// 批量三合一重铸：loot_ids 平铺，每 3 个一组 craft（EMOTE 用 forge，其余用 reroll）
pub async fn reroll_loot(
    client: &LcuClient,
    loot_ids: &[String],
) -> Result<Vec<LootActionProgress>, String> {
    let ids: Vec<String> = loot_ids
        .iter()
        .take(MAX_LOOP_COUNT as usize)
        .cloned()
        .collect();
    if ids.is_empty() || !ids.len().is_multiple_of(3) {
        return Err("重铸必须提供 3 的倍数个物品".to_string());
    }
    let total = (ids.len() / 3) as i32;
    let mut progress = Vec::new();
    for (g, group) in ids.chunks_exact(3).enumerate() {
        let current = g as i32 + 1;
        let keyword = if group[0].to_ascii_uppercase().contains("EMOTE") {
            "forge"
        } else {
            "reroll"
        };
        let result: Result<Value, String> = async {
            let recipe_name = find_recipe_name(client, &group[0], keyword).await?;
            craft_loot(client, &recipe_name, group, 1).await
        }
        .await;
        progress.push(make_progress(current, total, &result, &group[0]));
    }
    Ok(progress)
}

/// 批量升级 / 解锁：永久道具走 redeem；碎片走配方 craft 并携带精粹材料
pub async fn upgrade_loot(
    client: &LcuClient,
    items: &[DisenchantItem],
) -> Result<Vec<LootActionProgress>, String> {
    let jobs: Vec<&DisenchantItem> = items.iter().take(MAX_BATCH_ITEMS).collect();
    let total = jobs.len() as i32;
    let mut progress = Vec::new();
    for (idx, job) in jobs.iter().enumerate() {
        let current = idx as i32 + 1;
        let loot_upper = job.loot_id.to_ascii_uppercase();
        // 不含 SHARD / RENTAL 的视为永久道具，直接调用兑换接口
        let is_permanent = !loot_upper.contains("SHARD") && !loot_upper.contains("RENTAL");
        let result: Result<Value, String> = if is_permanent {
            redeem_loot(client, &job.loot_id, job.count)
                .await
                .map(|_| Value::Null)
        } else {
            upgrade_shard(client, job).await
        };
        progress.push(make_progress(current, total, &result, &job.loot_id));
    }
    Ok(progress)
}

/// 碎片升级：查升级配方 + 递归收集精粹材料，STATSTONE 逐个调用
async fn upgrade_shard(client: &LcuClient, job: &DisenchantItem) -> Result<Value, String> {
    let (recipe_name, extra) =
        find_upgrade_recipe_info(client, &job.loot_id, job.upgrade_recipe_name.as_deref()).await?;

    // body = [lootId, CURRENCY_champion?, CURRENCY_cosmetic?]
    let mut body = vec![job.loot_id.clone()];
    body.extend(extra);

    let is_statstone = job.loot_id.to_ascii_uppercase().contains("STATSTONE");
    // 永恒星碑配方不支持 repeat>1：每次 repeat=1，循环 count 次；其余一次 craft 带 repeat=count
    let (repeat, times) = if is_statstone {
        (1, clamp_count(job.count))
    } else {
        (clamp_count(job.count), 1)
    };

    let mut last = Value::Null;
    for _ in 0..times {
        last = craft_loot(client, &recipe_name, &body, repeat).await?;
    }
    Ok(last)
}

/// 查找升级 / 解锁配方名与所需精粹材料（优先采用前端传入的精确配方名）
async fn find_upgrade_recipe_info(
    client: &LcuClient,
    loot_id: &str,
    preferred: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    let raw = fetch_recipes_raw(client, loot_id).await?;

    // 1. 精确配方名优先
    let selected: Option<&Value> = preferred.filter(|name| !name.is_empty()).and_then(|name| {
        raw.iter()
            .find(|r| r.get("recipeName").and_then(|v| v.as_str()) == Some(name))
    });

    // 2. 关键字兜底：permanent → upgrade → open → claim → activate → unlock
    let selected = selected.or_else(|| {
        [
            "permanent",
            "upgrade",
            "open",
            "claim",
            "activate",
            "unlock",
        ]
        .iter()
        .copied()
        .find_map(|kw| {
            raw.iter().find(|r| {
                r.get("recipeName")
                    .and_then(|v| v.as_str())
                    .is_some_and(|name| name.to_ascii_lowercase().contains(kw))
            })
        })
    });

    let recipe = selected.ok_or_else(|| format!("未找到 {} 的升级/解锁配方", loot_id))?;
    let recipe_name = recipe
        .get("recipeName")
        .and_then(|v| v.as_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "配方缺少 recipeName 字段".to_string())?
        .to_string();

    // 递归扫描配方 JSON，收集 CURRENCY_champion / CURRENCY_cosmetic 精粹材料
    let mut extra = Vec::new();
    collect_currency_ingredients(recipe, &mut extra);
    extra.sort();
    extra.dedup();
    Ok((recipe_name, extra))
}

/// 递归收集配方中出现的精粹货币（CURRENCY_champion / CURRENCY_cosmetic）
fn collect_currency_ingredients(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            if s == "CURRENCY_champion" || s == "CURRENCY_cosmetic" {
                out.push(s.clone());
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_currency_ingredients(v, out);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                collect_currency_ingredients(v, out);
            }
        }
        _ => {}
    }
}
