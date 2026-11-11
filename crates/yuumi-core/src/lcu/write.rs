//! OP.GG 符文页与装备页的 LCU 写入
//!
//! 对照 Tauri 版 `src-tauri/src/tools.rs` 的 `apply_rune_page_core` 与 `apply_item_set`，
//! 按 OP.GG 方案文档 L03 收紧两处旧行为：
//!
//! 1. 符文：旧实现用 `.ok()` 吞掉当前页读取失败后继续删除替换；
//!    这里要求「读取当前页 → 取到可删除页快照 → 删除成功」三者齐全才继续，
//!    任一环节失败都中止，避免在无法恢复的情况下覆盖用户符文页。
//! 2. 装备：旧实现在读取已有 sets 失败时回退成空对象整写，会丢掉用户全部装备页；
//!    这里读取失败或返回形状异常直接报错。

use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

use super::client::LcuClient;

/// 应用符文页所需的参数
#[derive(Debug, Clone, Copy)]
pub struct RunePageParams<'a> {
    pub name: &'a str,
    pub primary_style_id: i32,
    pub sub_style_id: i32,
    pub selected_perk_ids: &'a [i32],
}

/// 装备页单条装备
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ItemSetItem {
    pub id: String,
    pub count: u32,
}

/// 装备页分组
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ItemSetBlockPayload {
    #[serde(rename = "type")]
    pub block_type: String,
    pub items: Vec<ItemSetItem>,
}

impl From<&crate::opgg::ItemSetBlock> for ItemSetBlockPayload {
    fn from(block: &crate::opgg::ItemSetBlock) -> Self {
        Self {
            block_type: block.block_type.clone(),
            items: block
                .items
                .iter()
                .map(|item| ItemSetItem {
                    id: item.id.clone(),
                    count: item.count,
                })
                .collect(),
        }
    }
}

/// 生成装备页唯一标识：时间戳 + 进程内自增序号，避免额外引入 uuid 依赖
fn next_item_set_uid() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let millis = chrono::Utc::now().timestamp_millis();
    format!("yuumi-{millis:x}-{sequence:x}")
}

/// 解析 `LcuClient` 错误文本中的 HTTP 状态码。
///
/// `LcuClient::request` 在非 2xx 时返回 `LCU 返回错误 [<code>]: <body>`，
/// 注意日志里打印的是另一种格式（`状态码: <code>`），两者不可混用。
fn lcu_status_code(error: &str) -> Option<u16> {
    let rest = error.strip_prefix("LCU 返回错误 [")?;
    let end = rest.find(']')?;
    rest[..end].trim().parse().ok()
}

/// 判断 LCU 错误是否为「资源不存在」
fn is_not_found(error: &str) -> bool {
    lcu_status_code(error) == Some(404)
}

/// 读取当前符文页的可删除页 ID 与其快照。
///
/// 返回 `Ok(None)` 表示当前页不可删除（无需腾位），`Err` 表示无法安全判断，
/// 调用方应据此中止替换。
///
/// `404 Perk Page not found` 表示客户端当前没有符文页：此时无需腾位，
/// 直接新建即可，不能与「读到了页但取不到快照」混为一谈。
async fn current_page_snapshot(client: &LcuClient) -> Result<Option<Value>, String> {
    let current = match client.get("/lol-perks/v1/currentpage").await {
        Ok(value) => value,
        Err(err) if is_not_found(&err) => {
            log::info!("[OPGG] 客户端当前没有符文页，直接新建");
            return Ok(None);
        }
        Err(err) => return Err(format!("读取当前符文页失败: {err}")),
    };

    let deletable = current
        .get("isDeletable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let page_id = current.get("id").and_then(Value::as_i64).unwrap_or(0);
    if !deletable || page_id <= 0 {
        return Ok(None);
    }

    let snapshot = client
        .get(&format!("/lol-perks/v1/pages/{page_id}"))
        .await
        .map_err(|err| format!("读取原符文页快照失败，已中止替换: {err}"))?;
    Ok(Some(snapshot))
}

/// 应用符文页：先腾出可删除页的页位，再创建并设为当前页。
///
/// 新建失败时尝试用快照恢复原页，并把恢复结果一并带回错误信息。
pub async fn apply_rune_page(client: &LcuClient, params: RunePageParams<'_>) -> Result<(), String> {
    let snapshot = current_page_snapshot(client).await?;

    let mut restored_page: Option<Value> = None;
    if let Some(snapshot) = snapshot {
        let page_id = snapshot.get("id").and_then(Value::as_i64).unwrap_or(0);
        if page_id > 0 {
            client
                .delete(&format!("/lol-perks/v1/pages/{page_id}"))
                .await
                .map_err(|err| format!("删除原符文页失败，已中止替换: {err}"))?;
        }
        restored_page = Some(snapshot);
    }

    let body = json!({
        "name": params.name,
        "primaryStyleId": params.primary_style_id,
        "subStyleId": params.sub_style_id,
        "selectedPerkIds": params.selected_perk_ids,
        "current": true,
    });

    match client.post("/lol-perks/v1/pages", Some(body)).await {
        Ok(_) => {
            log::info!("[OPGG] 符文页已应用: {}", params.name);
            Ok(())
        }
        Err(err) => {
            if let Some(mut snapshot) = restored_page {
                log::warn!("[OPGG] 新建符文页失败 ({err})，尝试恢复原页");
                if let Some(object) = snapshot.as_object_mut() {
                    object.remove("id");
                    object.remove("current");
                }
                match client.post("/lol-perks/v1/pages", Some(snapshot)).await {
                    Ok(_) => log::info!("[OPGG] 原符文页已恢复"),
                    Err(restore_err) => {
                        log::error!("[OPGG] 恢复原符文页失败: {restore_err}");
                        return Err(format!("{err}；恢复原符文页也失败: {restore_err}"));
                    }
                }
            }
            Err(err)
        }
    }
}

/// 从当前召唤师对象中取出数值型 ID（兼容数字与数字字符串）
fn numeric_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|field| {
        field
            .as_i64()
            .or_else(|| field.as_str().and_then(|text| text.parse().ok()))
    })
}

/// 写入装备页：保留非 Yuumi 装备页，替换旧 Yuumi 导入项。
pub async fn apply_item_set(
    client: &LcuClient,
    champion_id: i32,
    title: &str,
    blocks: &[ItemSetBlockPayload],
) -> Result<String, String> {
    let summoner = client
        .get("/lol-summoner/v1/current-summoner")
        .await
        .map_err(|err| format!("读取当前召唤师失败: {err}"))?;

    let summoner_id = numeric_field(&summoner, "summonerId")
        .or_else(|| numeric_field(&summoner, "id"))
        .ok_or_else(|| "无法获取当前召唤师 ID".to_string())?;
    let account_id = numeric_field(&summoner, "accountId").unwrap_or(summoner_id);

    let sets_path = format!("/lol-item-sets/v1/item-sets/{summoner_id}/sets");
    // 读取失败或形状异常时直接中止，绝不整写覆盖用户装备页
    let mut root = client
        .get(&sets_path)
        .await
        .map_err(|err| format!("读取现有装备页失败，已中止导入: {err}"))?;
    if !root.is_object() {
        return Err("现有装备页返回形状异常，已中止导入".to_string());
    }
    let existing = root
        .get("itemSets")
        .and_then(Value::as_array)
        .ok_or_else(|| "现有装备页缺少 itemSets 数组，已中止导入".to_string())?;

    let mut item_sets: Vec<Value> = existing
        .iter()
        .filter(|set| {
            let title = set.get("title").and_then(Value::as_str).unwrap_or("");
            !title.starts_with("[Yuumi]")
        })
        .cloned()
        .collect();

    item_sets.push(json!({
        "uid": next_item_set_uid(),
        "title": title,
        "associatedChampions": [champion_id],
        "associatedMaps": [11, 12],
        "blocks": blocks,
        "map": "any",
        "mode": "any",
        "preferredItemSlots": [],
        "sortrank": 1,
        "startedFrom": "blank",
        "type": "custom"
    }));

    if let Some(object) = root.as_object_mut() {
        object.insert("itemSets".to_string(), Value::Array(item_sets));
        if object.get("accountId").map_or(true, Value::is_null) {
            object.insert("accountId".to_string(), json!(account_id));
        }
        object.insert(
            "timestamp".to_string(),
            json!(chrono::Utc::now().timestamp_millis()),
        );
    }

    // LCU 的 sets 接口必须用 PUT 才会真正持久化，POST 虽返回 200 但不落库
    client
        .put(&sets_path, Some(root))
        .await
        .map_err(|err| format!("保存装备页失败: {err}"))?;
    log::info!("[OPGG] 装备页已写入: {title}");
    Ok(format!("装备页应用成功: {title}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_field_accepts_number_and_string() {
        let value = json!({ "summonerId": 42, "accountId": "7" });
        assert_eq!(numeric_field(&value, "summonerId"), Some(42));
        assert_eq!(numeric_field(&value, "accountId"), Some(7));
        assert_eq!(numeric_field(&value, "missing"), None);
    }

    #[test]
    fn not_found_detection_parses_returned_error_format() {
        // 调用方拿到的是 `LCU 返回错误 [<code>]: <body>`，不是日志里的 `状态码: <code>`
        assert!(is_not_found(
            "LCU 返回错误 [404]: {\"errorCode\":\"RPC_ERROR\",\"message\":\"Perk Page not found.\"}"
        ));
        assert!(!is_not_found("LCU 返回错误 [500]: {}"));
        assert!(!is_not_found("LCU 并发许可等待超时: GET /x"));
        assert!(!is_not_found("连接已断开"));
        assert_eq!(lcu_status_code("LCU 返回错误 [403]: {}"), Some(403));
    }

    #[test]
    fn item_set_uid_is_unique() {
        let first = next_item_set_uid();
        let second = next_item_set_uid();
        assert_ne!(first, second);
        assert!(first.starts_with("yuumi-"));
    }

    #[test]
    fn item_set_block_serializes_with_type_field() {
        let block = ItemSetBlockPayload {
            block_type: "核心装备".to_string(),
            items: vec![ItemSetItem {
                id: "6655".to_string(),
                count: 1,
            }],
        };
        let json = serde_json::to_value(&block).expect("分组应可序列化");
        assert_eq!(json["type"], "核心装备");
        assert_eq!(json["items"][0]["id"], "6655");
        assert_eq!(json["items"][0]["count"], 1);
    }
}
