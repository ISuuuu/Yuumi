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
