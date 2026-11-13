//! 战利品动作服务层（只读配方查询 + LCU 写操作 + 批量编排）
//!
//! 全部对外函数返回 `Result<T, String>`，统一复用 `LcuClient::get/post`。
//! 本模块只提供纯函数：不启动后台任务、不派发事件；每条动作完成时通过 `on_progress`
//! 回调即时上报（对照 Vue 的 `loot-*-progress` 事件），最终仍以 `Vec<LootActionProgress>` 聚合返回。

use crate::lcu::client::LcuClient;
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 逐条进度回调：动作层每完成一步立即调用，用于上层实时广播
pub type LootProgressSink<'a> = Option<&'a mut dyn FnMut(&LootActionProgress)>;

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

// ───────────────────────── 可开启战利品（配方 / 钥匙解析） ─────────────────────────

fn is_key_fragment_loot_id(loot_id: &str) -> bool {
    loot_id.eq_ignore_ascii_case("MATERIAL_key_fragment")
}

/// 对照 Vue `is_openable_loot`：宝箱 / 法球 / 胶囊 / 传送门 / 钥匙碎片
fn is_openable_loot(loot_type: &str, loot_id: &str) -> bool {
    let ty = loot_type.to_ascii_uppercase();
    let id = loot_id.to_ascii_uppercase();
    matches!(ty.as_str(), "CHEST" | "ORB" | "PORTAL")
        || id.contains("ORB")
        || id.contains("CHEST")
        || (ty == "MATERIAL"
            && (id.contains("ORB")
                || id.contains("CHEST")
                || id.contains("CAPSULE")
                || is_key_fragment_loot_id(loot_id)))
}

/// 从配方槽位中查找钥匙槽位，返回 (key_loot_id, need_key)
fn find_key_slot_loot_id(recipe: &Value) -> Option<String> {
    let slots = recipe.get("slots").and_then(|v| v.as_array())?;
    if slots.len() <= 1 {
        return None;
    }
    let key_slot = slots.iter().find(|slot| {
        if let Some(loot_ids) = slot.get("lootIds").and_then(|v| v.as_array()) {
            if loot_ids.iter().any(|id| {
                id.as_str()
                    .is_some_and(|s| s.to_lowercase().contains("key"))
            }) {
                return true;
            }
        }
        slot.get("slotRules")
            .and_then(|sr| sr.get("queryValue"))
            .and_then(|qv| qv.as_str())
            .is_some_and(|q| q.to_lowercase().contains("key"))
    })?;

    let from_loot_ids = key_slot
        .get("lootIds")
        .and_then(|v| v.as_array())
        .and_then(|ids| {
            ids.iter().find_map(|id| {
                id.as_str()
                    .filter(|s| s.to_lowercase().contains("key"))
                    .map(|s| s.to_string())
            })
        });
    if from_loot_ids.is_some() {
        return from_loot_ids;
    }
    key_slot
        .get("slotRules")
        .and_then(|sr| sr.get("queryValue"))
        .and_then(|qv| qv.as_str())
        .map(|s| s.to_string())
}

/// 拉取当前账号可开启的战利品（含真实配方名、是否需要钥匙、钥匙持有量）
///
/// 对照 Vue `get_openable_loots`：LCU 不提供「用哪个配方开启」的字段，只能逐个查询
/// `recipes/initial-item/{lootId}` 反推，因此这里以受限并发拉取。
pub async fn fetch_openable_loots(client: &LcuClient) -> Result<Vec<OpenableLoot>, String> {
    let raw = client.get("/lol-loot/v1/player-loot").await?;
    let arr = raw
        .as_array()
        .ok_or_else(|| "战利品数据格式错误，期望数组".to_string())?;

    let mut loot_counts = std::collections::HashMap::new();
    for item in arr {
        if let (Some(loot_id), Some(count)) = (
            item.get("lootId").and_then(|v| v.as_str()),
            item.get("count").and_then(|v| v.as_i64()),
        ) {
            loot_counts.insert(loot_id.to_string(), count as i32);
        }
    }

    let mut candidates: Vec<(String, String, i32, Option<String>)> = Vec::new();
    for item in arr {
        let loot_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let loot_id = item
            .get("lootId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let count = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        if loot_id.is_empty() || count <= 0 || !is_openable_loot(loot_type, loot_id) {
            continue;
        }

        let raw_name = item
            .get("localizedName")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                item.get("itemDesc")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or(loot_id)
            .to_string();
        let name = super::loot::normalize_loot_name(loot_id, &raw_name);
        let tile_path = item
            .get("tilePath")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| item.get("imagePath").and_then(|v| v.as_str()))
            .map(|s| s.to_string());

        candidates.push((loot_id.to_string(), name, count, tile_path));
    }

    let loot_counts = std::sync::Arc::new(loot_counts);
    let arr = std::sync::Arc::new(arr.clone());
    let results: Vec<Option<OpenableLoot>> = stream::iter(candidates)
        .map(|(loot_id, name, count, tile_path)| {
            let loot_client = client.clone();
            let counts = loot_counts.clone();
            let all = arr.clone();
            async move {
                let recipes = fetch_recipes_raw(&loot_client, &loot_id).await.ok()?;
                let recipe = recipes.iter().find(|r| {
                    r.get("recipeName")
                        .and_then(|v| v.as_str())
                        .is_some_and(|n| !n.is_empty())
                        && r.get("slots").and_then(|v| v.as_array()).is_some()
                })?;
                let recipe_name = recipe
                    .get("recipeName")
                    .and_then(|v| v.as_str())?
                    .to_string();
                let key_loot_id = find_key_slot_loot_id(recipe);
                let key_count = key_loot_id
                    .as_deref()
                    .and_then(|id| counts.get(id).copied())
                    .unwrap_or(0);
                let key_name = key_loot_id.as_deref().map(|id| {
                    all.iter()
                        .find(|item| item.get("lootId").and_then(|v| v.as_str()) == Some(id))
                        .and_then(|item| item.get("itemDesc").and_then(|v| v.as_str()))
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            if id.to_lowercase().contains("premium") {
                                "杰作钥匙".to_string()
                            } else {
                                "海克斯科技钥匙".to_string()
                            }
                        })
                });

                Some(OpenableLoot {
                    loot_id,
                    name,
                    count,
                    recipe_name,
                    need_key: key_loot_id.is_some(),
                    key_loot_id,
                    key_count,
                    key_name,
                    tile_path,
                })
            }
        })
        .buffer_unordered(8)
        .collect()
        .await;

    Ok(results.into_iter().flatten().collect())
}

/// 开启优先级（数字越小越先开启，对照 Vue `lootPriorityIndex`）
pub fn loot_priority_index(loot_id: &str) -> i32 {
    let id = loot_id.to_ascii_uppercase();
    if id == "CHEST_PROMOTION" {
        0
    } else if id == "CHEST_CHAMPION_MASTERY" || id == "CHEST_GENERIC" || id == "CHEST_HEXTECH" {
        1
    } else if id.contains("ORB") {
        2
    } else if id.contains("CAPSULE") {
        3
    } else {
        4
    }
}

/// 智能一键开启的分批计划（对照 Vue `buildSmartOpenBatches`）
///
/// 顺序固定为：先用碎片锻造钥匙 → 开启无需钥匙的宝箱/法球 → 按优先级消耗钥匙开启剩余宝箱。
pub fn build_smart_open_batches(
    openables: &[OpenableLoot],
    total_key_count: i32,
) -> Vec<OpenBatchItem> {
    let mut batches = Vec::new();

    let fragment = openables
        .iter()
        .find(|loot| is_key_fragment_loot_id(&loot.loot_id));
    let mut forged_keys = 0;
    if let Some(fragment) = fragment {
        if fragment.count >= 3 {
            forged_keys = fragment.count / 3;
            batches.push(OpenBatchItem {
                loot_id: fragment.loot_id.clone(),
                name: fragment.name.clone(),
                count: forged_keys,
                recipe_name: fragment.recipe_name.clone(),
                ingredients: vec![fragment.loot_id.clone()],
            });
        }
    }

    let mut remaining_keys = total_key_count.max(0) + forged_keys;

    for loot in openables
        .iter()
        .filter(|l| !l.need_key && l.count > 0 && !is_key_fragment_loot_id(&l.loot_id))
    {
        batches.push(OpenBatchItem {
            loot_id: loot.loot_id.clone(),
            name: loot.name.clone(),
            count: loot.count,
            recipe_name: loot.recipe_name.clone(),
            ingredients: vec![loot.loot_id.clone()],
        });
    }

    let mut key_needed: Vec<&OpenableLoot> = openables
        .iter()
        .filter(|l| l.need_key && l.count > 0 && !is_key_fragment_loot_id(&l.loot_id))
        .collect();
    key_needed.sort_by_key(|loot| loot_priority_index(&loot.loot_id));

    for loot in key_needed {
        if remaining_keys <= 0 {
            break;
        }
        let count = loot.count.min(remaining_keys);
        if count <= 0 {
            continue;
        }
        let mut ingredients = vec![loot.loot_id.clone()];
        if let Some(key) = &loot.key_loot_id {
            ingredients.push(key.clone());
        }
        batches.push(OpenBatchItem {
            loot_id: loot.loot_id.clone(),
            name: loot.name.clone(),
            count,
            recipe_name: loot.recipe_name.clone(),
            ingredients,
        });
        remaining_keys -= count;
    }

    batches
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

/// 上报一条进度（同时入聚合列表，保持返回值兼容）
fn push_progress(
    progress: &mut Vec<LootActionProgress>,
    sink: &mut LootProgressSink<'_>,
    item: LootActionProgress,
) {
    if let Some(cb) = sink.as_deref_mut() {
        cb(&item);
    }
    progress.push(item);
}

/// 单个开启 / 合成：对同一配方循环 repeat_count 次 craft（每次 repeat=1）
pub async fn batch_open_loots(
    client: &LcuClient,
    recipe_name: &str,
    ingredients: &[String],
    repeat_count: i32,
    mut on_progress: LootProgressSink<'_>,
) -> Result<Vec<LootActionProgress>, String> {
    let total = clamp_count(repeat_count);
    let mut progress = Vec::new();
    for i in 1..=total {
        let result = craft_loot(client, recipe_name, ingredients, 1).await;
        push_progress(
            &mut progress,
            &mut on_progress,
            make_progress(i, total, &result, recipe_name),
        );
    }
    Ok(progress)
}

/// 智能一键全部开启：按分批计划的顺序逐条目按 count 循环 craft（每次 repeat=1）
pub async fn smart_open_all_loots(
    client: &LcuClient,
    batches: &[OpenBatchItem],
    mut on_progress: LootProgressSink<'_>,
) -> Result<Vec<LootActionProgress>, String> {
    let sorted: Vec<OpenBatchItem> = batches.iter().take(MAX_BATCH_ITEMS).cloned().collect();

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
            push_progress(
                &mut progress,
                &mut on_progress,
                make_progress(current, total, &result, &item.name),
            );
        }
    }
    Ok(progress)
}

/// 批量分解：查含 disenchant 关键字的配方，按 count 作为 repeat 一次 craft
pub async fn disenchant_loot(
    client: &LcuClient,
    items: &[DisenchantItem],
    mut on_progress: LootProgressSink<'_>,
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
        push_progress(
            &mut progress,
            &mut on_progress,
            make_progress(current, total, &result, &job.loot_id),
        );
    }
    Ok(progress)
}

/// 批量三合一重铸：loot_ids 平铺，每 3 个一组 craft（EMOTE 用 forge，其余用 reroll）
pub async fn reroll_loot(
    client: &LcuClient,
    loot_ids: &[String],
    mut on_progress: LootProgressSink<'_>,
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
        push_progress(
            &mut progress,
            &mut on_progress,
            make_progress(current, total, &result, &group[0]),
        );
    }
    Ok(progress)
}

/// 批量升级 / 解锁：永久道具走 redeem；碎片走配方 craft 并携带精粹材料
pub async fn upgrade_loot(
    client: &LcuClient,
    items: &[DisenchantItem],
    mut on_progress: LootProgressSink<'_>,
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
        push_progress(
            &mut progress,
            &mut on_progress,
            make_progress(current, total, &result, &job.loot_id),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn openable(
        loot_id: &str,
        need_key: bool,
        count: i32,
        key: Option<&str>,
        key_count: i32,
    ) -> OpenableLoot {
        OpenableLoot {
            loot_id: loot_id.to_string(),
            name: loot_id.to_string(),
            count,
            recipe_name: format!("{}_OPEN", loot_id),
            need_key,
            key_loot_id: key.map(|k| k.to_string()),
            key_count,
            key_name: None,
            tile_path: None,
        }
    }

    #[test]
    fn smart_open_forges_key_fragments_first() {
        let openables = vec![
            openable("CHEST_128", false, 2, None, 0),
            openable("MATERIAL_key_fragment", false, 7, None, 0),
        ];
        let batches = build_smart_open_batches(&openables, 0);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].loot_id, "MATERIAL_key_fragment");
        assert_eq!(batches[0].count, 2);
        assert_eq!(batches[1].loot_id, "CHEST_128");
        assert_eq!(batches[1].count, 2);
    }

    #[test]
    fn smart_open_consumes_keys_by_priority() {
        let openables = vec![
            openable("CHEST_generic", true, 3, Some("MATERIAL_key"), 2),
            openable("CHEST_promotion", true, 1, Some("MATERIAL_key"), 2),
            openable("CHEST_128", false, 1, None, 0),
        ];
        let batches = build_smart_open_batches(&openables, 2);
        // 无钥匙宝箱先开
        assert_eq!(batches[0].loot_id, "CHEST_128");
        // 钥匙按优先级分配：CHEST_promotion 优先级更高，先消耗 1 把
        assert_eq!(batches[1].loot_id, "CHEST_promotion");
        assert_eq!(batches[1].count, 1);
        assert_eq!(
            batches[1].ingredients,
            vec!["CHEST_promotion".to_string(), "MATERIAL_key".to_string()]
        );
        // 剩余 1 把钥匙给 CHEST_generic，实际只开 1 个
        assert_eq!(batches[2].loot_id, "CHEST_generic");
        assert_eq!(batches[2].count, 1);
    }

    #[test]
    fn smart_open_skips_when_out_of_keys() {
        let openables = vec![openable("CHEST_generic", true, 3, Some("MATERIAL_key"), 0)];
        assert!(build_smart_open_batches(&openables, 0).is_empty());
    }
}
