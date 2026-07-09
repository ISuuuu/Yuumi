use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::{build_auth_header, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LootProgressEvent {
    pub current: i32,
    pub total: i32,
    pub success: bool,
    pub reward_name: String,
    pub error_msg: Option<String>,
    pub item_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenBatchItem {
    pub loot_id: String,
    pub name: String,
    pub count: i32,
    pub recipe_name: String,
    pub ingredients: Vec<String>,
}

/// 碎片库存条目（英雄/皮肤/表情/守卫皮肤/召唤师图标）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LootItem {
    pub loot_id: String,
    pub loot_name: String,
    pub item_desc: String,
    pub display_categories: String,
    pub loot_type: String,
    pub rarity: String,
    pub count: i32,
    pub value: i32,
    pub disenchant_value: i32,
    pub item_status: String,
    pub tile_path: Option<String>,
    pub upgrade_recipe_name: String,
    pub upgrade_essence_cost: i32,
}

/// 批量分解请求条目
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisenchantItem {
    pub loot_id: String,
    pub count: i32,
    pub upgrade_recipe_name: Option<String>,
}

/// 碎片操作进度广播事件（分解/重随复用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionProgressEvent {
    pub current: i32,
    pub total: i32,
    pub success: bool,
    pub loot_name: String,
    pub reward_desc: String,
    pub error_msg: Option<String>,
}

/// 计算战利品开启优先级（数字越小优先级越高）
fn loot_priority(loot_id: &str) -> i32 {
    // 优先级：宝箱 > 海克斯宝箱 > 法球/胶囊 > 其他
    if loot_id.contains("CHEST") && !loot_id.contains("hextech") && !loot_id.contains("premium") {
        0 // 普通宝箱 — 最高优先级
    } else if loot_id == "CHEST_hextech" || loot_id.contains("hextech") {
        1 // 海克斯宝箱
    } else if loot_id.contains("ORB") || loot_id.contains("orb") {
        2 // 法球
    } else if loot_id.contains("CAPSULE") || loot_id.contains("capsule") {
        3 // 胶囊
    } else {
        4 // 其他
    }
}

/// 1. 获取当前所有可开启的战利品
#[tauri::command]
pub async fn get_openable_loots(
    app_state: State<'_, AppState>,
) -> Result<Vec<OpenableLoot>, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    // 1. 获取 player-loot
    let url = format!("{}/lol-loot/v1/player-loot", base);
    let loot_resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let loots: Vec<serde_json::Value> = loot_resp.json().await.map_err(|e| e.to_string())?;

    // 建立所有战利品数量的哈希映射
    let mut loot_counts = std::collections::HashMap::new();
    for item in &loots {
        if let (Some(loot_id), Some(count)) = (
            item.get("lootId").and_then(|v| v.as_str()),
            item.get("count").and_then(|v| v.as_i64()),
        ) {
            loot_counts.insert(loot_id.to_string(), count as i32);
        }
    }

    let mut result = Vec::new();

    // 2. 筛选可开启的战利品（箱子、法球等）并拉取配方
    for item in loots.iter() {
        let loot_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let loot_id = item.get("lootId").and_then(|v| v.as_str()).unwrap_or("");
        let name = item
            .get("itemDesc")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(loot_id);
        let count = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let tile_path = item
            .get("tilePath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let is_openable = loot_type == "CHEST"
            || loot_type == "ORB"
            || loot_id.contains("ORB")
            || loot_id.contains("Chest")
            || loot_id.contains("Orb")
            || loot_type == "MATERIAL"
                && (loot_id.contains("ORB")
                    || loot_id.contains("CHEST")
                    || loot_id.contains("CAPSULE"));

        if !is_openable || count <= 0 {
            continue;
        }

        // 获取此 item 的配方
        let recipe_url = format!("{}/lol-loot/v1/recipes/initial-item/{}", base, loot_id);
        if let Ok(recipe_resp) = lcu
            .http_client
            .get(&recipe_url)
            .header("Authorization", &auth)
            .send()
            .await
        {
            if let Ok(recipes) = recipe_resp.json::<Vec<serde_json::Value>>().await {
                for r in recipes {
                    let recipe_name = r.get("recipeName").and_then(|v| v.as_str()).unwrap_or("");
                    let slots = r.get("slots").and_then(|v| v.as_array());

                    if !recipe_name.is_empty() {
                        let slots = match slots {
                            Some(s) => s,
                            None => continue,
                        };
                        let mut need_key = false;
                        let mut key_loot_id: Option<String> = None;
                        let mut key_count = 0;
                        let mut key_name = None;

                        if slots.len() > 1 {
                            need_key = true;

                            // 查找钥匙槽位 (同时支持新版 lootIds 列表和旧版 slotRules 规则匹配)
                            if let Some(key_slot) = slots.iter().find(|s| {
                                // 方式 A: 检查 slots 里的 lootIds 数组是否包含 key
                                if let Some(loot_ids) = s.get("lootIds").and_then(|v| v.as_array())
                                {
                                    if loot_ids.iter().any(|id| {
                                        id.as_str()
                                            .map(|s| {
                                                s.contains("key")
                                                    || s.contains("KEY")
                                                    || s.contains("Key")
                                            })
                                            .unwrap_or(false)
                                    }) {
                                        return true;
                                    }
                                }
                                // 方式 B: 检查旧版的 slotRules
                                s.get("slotRules")
                                    .and_then(|sr| sr.get("queryValue").and_then(|qv| qv.as_str()))
                                    .map(|q| {
                                        q.contains("key") || q.contains("KEY") || q.contains("Key")
                                    })
                                    .unwrap_or(false)
                            }) {
                                // 提取钥匙 ID
                                let mut kid = None;

                                // 优先从 lootIds 数组里提取第一个包含 key 的 ID
                                if let Some(loot_ids) =
                                    key_slot.get("lootIds").and_then(|v| v.as_array())
                                {
                                    kid = loot_ids.iter().find_map(|id| {
                                        id.as_str()
                                            .filter(|s| {
                                                s.contains("key")
                                                    || s.contains("KEY")
                                                    || s.contains("Key")
                                            })
                                            .map(|s| s.to_string())
                                    });
                                }

                                // 如果没有，再从 slotRules 提取
                                if kid.is_none() {
                                    kid = key_slot
                                        .get("slotRules")
                                        .and_then(|sr| sr.get("queryValue"))
                                        .and_then(|qv| qv.as_str())
                                        .map(|s| s.to_string());
                                }

                                if let Some(ref id) = kid {
                                    key_count = *loot_counts.get(id).unwrap_or(&0);

                                    // 查找钥匙对应的名字描述
                                    if let Some(key_item) = loots.iter().find(|item| {
                                        item.get("lootId").and_then(|v| v.as_str()) == Some(id)
                                    }) {
                                        key_name = key_item
                                            .get("itemDesc")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());
                                    } else {
                                        key_name = Some(if id.contains("premium") {
                                            "杰作钥匙".to_string()
                                        } else {
                                            "海克斯科技钥匙".to_string()
                                        });
                                    }
                                }
                                key_loot_id = kid;
                            }
                        }

                        result.push(OpenableLoot {
                            loot_id: loot_id.to_string(),
                            name: name.to_string(),
                            count,
                            recipe_name: recipe_name.to_string(),
                            need_key,
                            key_loot_id,
                            key_count,
                            key_name,
                            tile_path: tile_path.clone(),
                        });
                        break; // 取第一个开启配方即可
                    }
                }
            }
        }
    }

    Ok(result)
}

/// 2. 异步后台批量开启命令
#[tauri::command]
pub async fn batch_open_loots(
    recipe_name: String,
    ingredients: Vec<String>,
    repeat_count: i32,
    app_handle: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client.clone();
    drop(lock); // 尽快释放锁以防阻塞其他命令

    // 在后台协程中顺序执行开启
    crate::spawn_log_panic(async move {
        let url = format!("{}/lol-loot/v1/recipes/{}/craft", base, recipe_name);

        for i in 1..=repeat_count {
            // 每轮开启之间稍微休息一下（如 180ms），保持客户端 UI 与数据平滑同步
            tokio::time::sleep(std::time::Duration::from_millis(180)).await;

            let resp = http_client
                .post(&url)
                .header("Authorization", &auth)
                .json(&ingredients)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    let detail: serde_json::Value = r.json().await.unwrap_or_default();
                    let mut reward_name = String::new();
                    if let Some(added) = detail.get("added").and_then(|v| v.as_array()) {
                        let rewards: Vec<String> = added
                            .iter()
                            .map(|x| {
                                let player_loot = x.get("playerLoot");
                                let item_desc = player_loot
                                    .and_then(|pl| pl.get("itemDesc"))
                                    .and_then(|d| d.as_str())
                                    .unwrap_or("");
                                let loot_id = player_loot
                                    .and_then(|pl| pl.get("lootId"))
                                    .and_then(|id| id.as_str())
                                    .unwrap_or("");

                                if !item_desc.is_empty() {
                                    item_desc.to_string()
                                } else {
                                    match loot_id {
                                        "MATERIAL_key" => "海克斯科技钥匙".to_string(),
                                        "MATERIAL_key_fragment" => "海克斯科技钥匙碎片".to_string(),
                                        "MATERIAL_key_premium" => "杰作钥匙".to_string(),
                                        "CHEST_promotion" => "额外赠送宝箱".to_string(),
                                        "CHEST_champion_mastery" => "成就传送门".to_string(),
                                        "CHEST_generic" => "海克斯科技传送门".to_string(),
                                        "CHEST_premium" => "杰作宝箱".to_string(),
                                        "CURRENCY_mythic" => "神话精萃".to_string(),
                                        _ => {
                                            if !loot_id.is_empty() {
                                                loot_id.to_string()
                                            } else {
                                                "未知道具".to_string()
                                            }
                                        }
                                    }
                                }
                            })
                            .filter(|name| !name.is_empty() && name != "未知道具")
                            .collect();
                        reward_name = if rewards.is_empty() {
                            "未知道具".to_string()
                        } else {
                            rewards.join(", ")
                        };
                    }

                    let _ = app_handle.emit(
                        "loot-open-progress",
                        LootProgressEvent {
                            current: i,
                            total: repeat_count,
                            success: true,
                            reward_name,
                            error_msg: None,
                            item_name: None,
                        },
                    );
                }
                Ok(r) => {
                    let _ = app_handle.emit(
                        "loot-open-progress",
                        LootProgressEvent {
                            current: i,
                            total: repeat_count,
                            success: false,
                            reward_name: String::new(),
                            error_msg: Some(format!("HTTP 错误: {}", r.status())),
                            item_name: None,
                        },
                    );
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "loot-open-progress",
                        LootProgressEvent {
                            current: i,
                            total: repeat_count,
                            success: false,
                            reward_name: String::new(),
                            error_msg: Some(e.to_string()),
                            item_name: None,
                        },
                    );
                }
            }
        }
    });

    Ok("任务已推入后台开启队列".to_string())
}

/// 3. 智能一键开启：根据钥匙数量与优先级，自动分配开启批次
///    优先级：宝箱 > 海克斯宝箱 > 法球/胶囊 > 其他
#[tauri::command]
pub async fn smart_open_all_loots(
    items: Vec<OpenBatchItem>,
    app_handle: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client.clone();
    drop(lock);

    // 按优先级排序
    let mut sorted = items.clone();
    sorted.sort_by_key(|item| loot_priority(&item.loot_id));

    // 拆除 items 避免闭包捕获问题
    let items_arc = std::sync::Arc::new(sorted);

    crate::spawn_log_panic(async move {
        let mut global_counter = 0i32;
        let total_count: i32 = items_arc.iter().map(|i| i.count).sum();

        for item in items_arc.iter() {
            for _ in 0..item.count {
                global_counter += 1;
                tokio::time::sleep(std::time::Duration::from_millis(180)).await;

                let url = format!("{}/lol-loot/v1/recipes/{}/craft", base, item.recipe_name);

                let resp = http_client
                    .post(&url)
                    .header("Authorization", &auth)
                    .json(&item.ingredients)
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => {
                        let detail: serde_json::Value = r.json().await.unwrap_or_default();
                        let mut reward_name = String::new();
                        if let Some(added) = detail.get("added").and_then(|v| v.as_array()) {
                            let rewards: Vec<String> = added
                                .iter()
                                .map(|x| {
                                    let player_loot = x.get("playerLoot");
                                    let item_desc = player_loot
                                        .and_then(|pl| pl.get("itemDesc"))
                                        .and_then(|d| d.as_str())
                                        .unwrap_or("");
                                    if !item_desc.is_empty() {
                                        item_desc.to_string()
                                    } else {
                                        let lid = player_loot
                                            .and_then(|pl| pl.get("lootId"))
                                            .and_then(|id| id.as_str())
                                            .unwrap_or("");
                                        match lid {
                                            "MATERIAL_key" => "海克斯科技钥匙",
                                            "MATERIAL_key_fragment" => "海克斯科技钥匙碎片",
                                            "MATERIAL_key_premium" => "杰作钥匙",
                                            "CHEST_promotion" => "额外赠送宝箱",
                                            "CHEST_champion_mastery" => "成就传送门",
                                            "CHEST_generic" => "海克斯科技传送门",
                                            "CHEST_premium" => "杰作宝箱",
                                            "CURRENCY_mythic" => "神话精萃",
                                            _ => "",
                                        }
                                        .to_string()
                                    }
                                })
                                .filter(|n| !n.is_empty())
                                .collect();
                            reward_name = if rewards.is_empty() {
                                "未知道具".to_string()
                            } else {
                                rewards.join(", ")
                            };
                        }

                        let _ = app_handle.emit(
                            "loot-open-progress",
                            LootProgressEvent {
                                current: global_counter,
                                total: total_count,
                                success: true,
                                reward_name,
                                error_msg: None,
                                item_name: Some(item.name.clone()),
                            },
                        );
                    }
                    Ok(r) => {
                        let _ = app_handle.emit(
                            "loot-open-progress",
                            LootProgressEvent {
                                current: global_counter,
                                total: total_count,
                                success: false,
                                reward_name: String::new(),
                                error_msg: Some(format!("HTTP 错误: {}", r.status())),
                                item_name: Some(item.name.clone()),
                            },
                        );
                    }
                    Err(e) => {
                        let _ = app_handle.emit(
                            "loot-open-progress",
                            LootProgressEvent {
                                current: global_counter,
                                total: total_count,
                                success: false,
                                reward_name: String::new(),
                                error_msg: Some(e.to_string()),
                                item_name: Some(item.name.clone()),
                            },
                        );
                    }
                }
            }
        }
    });

    Ok("智能一键开启任务已推入后台队列".to_string())
}

/// 动态查询某碎片支持的特定动作配方名（按关键字匹配 recipeName）
/// 关键字如 "disenchant" / "reroll" / "forge"。避免硬编码配方名随版本失效。
async fn find_recipe_name(
    base: &str,
    auth: &str,
    http_client: &reqwest::Client,
    loot_id: &str,
    keyword: &str,
) -> Result<String, String> {
    let recipe_url = format!("{}/lol-loot/v1/recipes/initial-item/{}", base, loot_id);
    let resp = http_client
        .get(&recipe_url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("获取配方接口返回错误: {}", resp.status()));
    }

    let recipes: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
    for r in &recipes {
        if let Some(recipe_name) = r.get("recipeName").and_then(|v| v.as_str()) {
            if recipe_name.to_lowercase().contains(keyword) {
                return Ok(recipe_name.to_string());
            }
        }
    }

    Err(format!("未找到包含关键字 '{}' 的配方", keyword))
}

/// 4. 获取玩家所有碎片类战利品（排除材料/货币/箱子）
#[tauri::command]
pub async fn get_loot_inventory(app_state: State<'_, AppState>) -> Result<Vec<LootItem>, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    let url = format!("{}/lol-loot/v1/player-loot", base);
    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw_loots: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    // 获取 summoner_id 与已拥有皮肤列表并建立 Hash 集合，解决 LCU 中皮肤碎片 itemStatus 始终为 NONE 的问题
    let mut owned_skin_ids = std::collections::HashSet::new();
    let summoner_url = format!("{}/lol-summoner/v1/current-summoner", base);
    if let Ok(summoner_resp) = lcu
        .http_client
        .get(&summoner_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        if summoner_resp.status().is_success() {
            if let Ok(summoner_json) = summoner_resp.json::<serde_json::Value>().await {
                if let Some(summoner_id) = summoner_json.get("summonerId").and_then(|v| v.as_i64())
                {
                    let skins_url = format!(
                        "{}/lol-champions/v1/inventories/by-summoner/{}/skins",
                        base, summoner_id
                    );
                    if let Ok(skins_resp) = lcu
                        .http_client
                        .get(&skins_url)
                        .header("Authorization", &auth)
                        .send()
                        .await
                    {
                        if skins_resp.status().is_success() {
                            if let Ok(skins_json) =
                                skins_resp.json::<Vec<serde_json::Value>>().await
                            {
                                for s in skins_json {
                                    if let (Some(skin_id), Some(ownership)) =
                                        (s.get("id").and_then(|v| v.as_i64()), s.get("ownership"))
                                    {
                                        if ownership
                                            .get("owned")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false)
                                        {
                                            owned_skin_ids.insert(skin_id as i32);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut result = Vec::new();

    for item in raw_loots {
        let display_category = item
            .get("displayCategories")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let loot_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let loot_id = item.get("lootId").and_then(|v| v.as_str()).unwrap_or("");

        // 仅筛选碎片、表情、守卫、图标，排除材料(钥匙、传送门、精粹等)
        let is_shard = matches!(
            display_category,
            "CHAMPION" | "SKIN" | "EMOTE" | "WARDSKIN" | "SUMMONERICON"
        );
        let is_material = matches!(loot_type, "MATERIAL" | "CURRENCY" | "CHEST" | "ORB");

        if !is_shard || is_material {
            continue;
        }

        let count = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        if count <= 0 {
            continue;
        }

        let item_desc = item
            .get("itemDesc")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(loot_id)
            .to_string();
        let value = item.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let disenchant_value = item
            .get("disenchantValue")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let store_item_id = item
            .get("storeItemId")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let mut item_status = item
            .get("itemStatus")
            .and_then(|v| v.as_str())
            .unwrap_or("NONE")
            .to_string();

        if display_category == "SKIN"
            && store_item_id > 0
            && owned_skin_ids.contains(&store_item_id)
        {
            item_status = "OWNED".to_string();
        }
        let tile_path = item
            .get("tilePath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let rarity = item
            .get("rarity")
            .and_then(|v| v.as_str())
            .unwrap_or("DEFAULT")
            .to_string();
        let loot_name = item
            .get("lootName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let upgrade_recipe_name = item
            .get("upgradeRecipeName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let upgrade_essence_cost = item
            .get("upgradeEssenceValue")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        result.push(LootItem {
            loot_id: loot_id.to_string(),
            loot_name,
            item_desc,
            display_categories: display_category.to_string(),
            loot_type: loot_type.to_string(),
            rarity,
            count,
            value,
            disenchant_value,
            item_status,
            tile_path,
            upgrade_recipe_name,
            upgrade_essence_cost,
        });
    }

    Ok(result)
}

/// 5. 批量分解选中碎片（后台顺序执行，逐条广播进度）
#[tauri::command]
pub async fn disenchant_loot(
    items: Vec<DisenchantItem>,
    app_handle: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client.clone();
    drop(lock);

    crate::spawn_log_panic(async move {
        let total_jobs = items.len() as i32;

        for (current_job, job) in items.into_iter().enumerate() {
            let current_job = current_job as i32 + 1;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;

            // 1. 动态查找分解配方
            match find_recipe_name(&base, &auth, &http_client, &job.loot_id, "disenchant").await {
                Ok(recipe_name) => {
                    // 2. 执行分解请求
                    let craft_url = format!(
                        "{}/lol-loot/v1/recipes/{}/craft?repeat={}",
                        base, recipe_name, job.count
                    );
                    let body = vec![job.loot_id.clone()];

                    let craft_resp = http_client
                        .post(&craft_url)
                        .header("Authorization", &auth)
                        .json(&body)
                        .send()
                        .await;

                    match craft_resp {
                        Ok(r) if r.status().is_success() => {
                            let _ = app_handle.emit(
                                "loot-disenchant-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: true,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: format!("成功分解 {} 个", job.count),
                                    error_msg: None,
                                },
                            );
                        }
                        Ok(r) => {
                            let _ = app_handle.emit(
                                "loot-disenchant-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: false,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(format!("HTTP 错误: {}", r.status())),
                                },
                            );
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "loot-disenchant-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: false,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(e.to_string()),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "loot-disenchant-progress",
                        ActionProgressEvent {
                            current: current_job,
                            total: total_jobs,
                            success: false,
                            loot_name: job.loot_id,
                            reward_desc: String::new(),
                            error_msg: Some(format!("找不到配方: {}", e)),
                        },
                    );
                }
            }
        }
    });

    Ok("批量分解任务已推入后台队列".to_string())
}

/// 6. 批量三合一重随：输入平铺 loot_ids 列表（长度需为 3 的倍数，前端按类别分包）
///    每 3 个为一组，调用对应重随/锻造配方合成一个永久物品。
#[tauri::command]
pub async fn reroll_loot(
    loot_ids: Vec<String>,
    app_handle: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    if loot_ids.is_empty() || !loot_ids.len().is_multiple_of(3) {
        return Err("重随必须提供3的倍数个物品".to_string());
    }

    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client.clone();
    drop(lock);

    crate::spawn_log_panic(async move {
        let total_groups = (loot_ids.len() / 3) as i32;

        for g in 0..total_groups {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            let idx = (g * 3) as usize;
            let group_ingredients = vec![
                loot_ids[idx].clone(),
                loot_ids[idx + 1].clone(),
                loot_ids[idx + 2].clone(),
            ];

            // 表情用 forge 配方，其他用 reroll 配方
            let keyword = if group_ingredients[0].contains("EMOTE") {
                "forge"
            } else {
                "reroll"
            };

            match find_recipe_name(&base, &auth, &http_client, &group_ingredients[0], keyword).await
            {
                Ok(recipe_name) => {
                    let craft_url = format!("{}/lol-loot/v1/recipes/{}/craft", base, recipe_name);

                    let craft_resp = http_client
                        .post(&craft_url)
                        .header("Authorization", &auth)
                        .json(&group_ingredients)
                        .send()
                        .await;

                    match craft_resp {
                        Ok(r) if r.status().is_success() => {
                            let mut reward_name = "未知永久物品".to_string();
                            if let Ok(detail) = r.json::<serde_json::Value>().await {
                                if let Some(added) = detail.get("added").and_then(|v| v.as_array())
                                {
                                    let rewards: Vec<String> = added
                                        .iter()
                                        .map(|x| {
                                            x.get("playerLoot")
                                                .and_then(|pl| pl.get("itemDesc"))
                                                .and_then(|d| d.as_str())
                                                .unwrap_or("")
                                                .to_string()
                                        })
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    if !rewards.is_empty() {
                                        reward_name = rewards.join(", ");
                                    }
                                }
                            }

                            let _ = app_handle.emit(
                                "loot-reroll-progress",
                                ActionProgressEvent {
                                    current: g + 1,
                                    total: total_groups,
                                    success: true,
                                    loot_name: group_ingredients[0].clone(),
                                    reward_desc: reward_name,
                                    error_msg: None,
                                },
                            );
                        }
                        Ok(r) => {
                            let _ = app_handle.emit(
                                "loot-reroll-progress",
                                ActionProgressEvent {
                                    current: g + 1,
                                    total: total_groups,
                                    success: false,
                                    loot_name: group_ingredients[0].clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(format!("HTTP 错误: {}", r.status())),
                                },
                            );
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "loot-reroll-progress",
                                ActionProgressEvent {
                                    current: g + 1,
                                    total: total_groups,
                                    success: false,
                                    loot_name: group_ingredients[0].clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(e.to_string()),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "loot-reroll-progress",
                        ActionProgressEvent {
                            current: g + 1,
                            total: total_groups,
                            success: false,
                            loot_name: group_ingredients[0].clone(),
                            reward_desc: String::new(),
                            error_msg: Some(format!("找不到配方: {}", e)),
                        },
                    );
                }
            }
        }
    });

    Ok("三合一重随任务已推入后台队列".to_string())
}

/// 7. 批量升级选中项为永久物品
#[tauri::command]
pub async fn upgrade_loot(
    items: Vec<DisenchantItem>,
    app_handle: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client.clone();
    drop(lock);

    crate::spawn_log_panic(async move {
        let total_jobs = items.len() as i32;

        for (current_job, job) in items.into_iter().enumerate() {
            let current_job = current_job as i32 + 1;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;

            // 1. 获取升级配方名称 (优先使用前端传来的精确配方，无则用关键字兜底匹配)
            let recipe_result = if let Some(ref r_name) = job.upgrade_recipe_name {
                if !r_name.is_empty() {
                    Ok(r_name.clone())
                } else {
                    match find_recipe_name(&base, &auth, &http_client, &job.loot_id, "permanent")
                        .await
                    {
                        Ok(name) => Ok(name),
                        Err(_) => {
                            find_recipe_name(&base, &auth, &http_client, &job.loot_id, "upgrade")
                                .await
                        }
                    }
                }
            } else {
                match find_recipe_name(&base, &auth, &http_client, &job.loot_id, "permanent").await
                {
                    Ok(name) => Ok(name),
                    Err(_) => {
                        find_recipe_name(&base, &auth, &http_client, &job.loot_id, "upgrade").await
                    }
                }
            };

            match recipe_result {
                Ok(recipe_name) => {
                    // 2. 执行升级请求
                    let craft_url = format!(
                        "{}/lol-loot/v1/recipes/{}/craft?repeat={}",
                        base, recipe_name, job.count
                    );
                    // 皮肤碎片用橙精粹，英雄碎片用蓝精粹
                    let currency_id = if job.loot_id.to_uppercase().contains("SKIN") {
                        "CURRENCY_cosmetic"
                    } else {
                        "CURRENCY_champion"
                    };
                    let body = vec![job.loot_id.clone(), currency_id.to_string()];

                    let craft_resp = http_client
                        .post(&craft_url)
                        .header("Authorization", &auth)
                        .json(&body)
                        .send()
                        .await;

                    match craft_resp {
                        Ok(r) if r.status().is_success() => {
                            let _ = app_handle.emit(
                                "loot-upgrade-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: true,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: format!("成功升级 {} 个", job.count),
                                    error_msg: None,
                                },
                            );
                        }
                        Ok(r) => {
                            let _ = app_handle.emit(
                                "loot-upgrade-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: false,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(format!("HTTP 错误: {}", r.status())),
                                },
                            );
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "loot-upgrade-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: false,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(e.to_string()),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "loot-upgrade-progress",
                        ActionProgressEvent {
                            current: current_job,
                            total: total_jobs,
                            success: false,
                            loot_name: job.loot_id,
                            reward_desc: String::new(),
                            error_msg: Some(format!("找不到升级配方: {}", e)),
                        },
                    );
                }
            }
        }
    });

    Ok("批量升级任务已推入后台队列".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EssenceBalances {
    pub blue_essence: i32,
    pub orange_essence: i32,
}

/// 8. 获取玩家蓝/橙精粹余额
#[tauri::command]
pub async fn get_essence_balances(
    app_state: State<'_, AppState>,
) -> Result<EssenceBalances, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client.clone();
    drop(lock);

    let url = format!("{}/lol-loot/v1/player-loot", base);
    let resp = http_client
        .get(&url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw_loots: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    let mut blue_essence = 0;
    let mut orange_essence = 0;

    for item in raw_loots {
        if let Some(loot_id) = item.get("lootId").and_then(|v| v.as_str()) {
            if loot_id == "CURRENCY_champion" {
                blue_essence = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            } else if loot_id == "CURRENCY_cosmetic" {
                orange_essence = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            }
        }
    }

    Ok(EssenceBalances {
        blue_essence,
        orange_essence,
    })
}
