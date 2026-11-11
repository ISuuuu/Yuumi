//! 选择器静态资源与英雄别称服务。
//!
//! - `SPELL_ICONS`：LCU GameData 预加载完成后写入的召唤师技能 ID → iconPath 映射，
//!   供工具箱技能选择器渲染图标（图标路径无固定模板，必须来自真实载荷）。
//! - `CHAMPION_KEYWORDS`：腾讯英雄别称/拼音检索库（对齐 Vue `championCache.ts` 的
//!   `fetchKeywords`），应用启动后拉取一次，失败静默回落到 `data.rs` 的静态拼音表。

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use yuumi_core::lcu::GameDataAssets;

/// 召唤师技能 ID → iconPath（GameData 预加载后写入，重连时刷新）
static SPELL_ICONS: LazyLock<RwLock<HashMap<i32, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 英雄 ID → 腾讯别称检索串（"拼音,别称,称号,英文名"），启动后台拉取一次
static CHAMPION_KEYWORDS: LazyLock<RwLock<HashMap<i32, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 腾讯英雄资料库（公网直连，与 Vue 版同源）
const TENCENT_HERO_LIST_URL: &str =
    "https://game.gtimg.cn/images/lol/act/img/js/heroList/hero_list.js";

/// 记录腾讯库是否已拉取成功，避免 LCU 重连时重复下载
static KEYWORDS_LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// GameData 预加载完成后写入召唤师技能图标映射
pub fn store_spell_icons(assets: &GameDataAssets) {
    if let Ok(mut map) = SPELL_ICONS.write() {
        *map = assets.spells.clone();
    }
}

/// 查询召唤师技能图标路径（未预加载或未知 ID 返回 None）
pub fn spell_icon_path(spell_id: i32) -> Option<String> {
    SPELL_ICONS
        .read()
        .ok()
        .and_then(|map| map.get(&spell_id).cloned())
        .filter(|path| !path.is_empty())
}

/// 查询英雄的腾讯别称检索串（未加载或未知 ID 返回空串）
pub fn champion_keywords(champion_id: i32) -> String {
    CHAMPION_KEYWORDS
        .read()
        .ok()
        .and_then(|map| map.get(&champion_id).cloned())
        .unwrap_or_default()
}

/// 启动后台拉取腾讯英雄别称/拼音库（已成功加载过则跳过；失败静默，回落静态拼音表）
pub fn spawn_fetch_tencent_keywords() {
    if KEYWORDS_LOADED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    tokio::spawn(async move {
        match fetch_tencent_keywords().await {
            Ok(map) if !map.is_empty() => {
                if let Ok(mut cache) = CHAMPION_KEYWORDS.write() {
                    *cache = map;
                }
                KEYWORDS_LOADED.store(true, std::sync::atomic::Ordering::Relaxed);
                log::info!(
                    "[ChampionAlias] 腾讯英雄别称/拼音库加载完毕 ({} 位)",
                    CHAMPION_KEYWORDS.read().map(|m| m.len()).unwrap_or(0)
                );
            }
            Ok(_) => log::warn!("[ChampionAlias] 腾讯英雄别称库响应为空，搜索回落静态拼音表"),
            Err(e) => log::warn!("[ChampionAlias] 腾讯英雄别称库拉取失败，搜索回落静态拼音表: {e}"),
        }
    });
}

/// 拉取并解析腾讯英雄别称库。返回 英雄ID → "keywords,title,alias" 检索串。
async fn fetch_tencent_keywords() -> Result<HashMap<i32, String>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(TENCENT_HERO_LIST_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let payload: TencentHeroList = response
        .json()
        .await
        .map_err(|e| format!("解析腾讯英雄库失败: {e}"))?;
    Ok(parse_tencent_heroes(&payload))
}

/// 腾讯 hero_list.js 载荷（仅消费字段子集；响应顶层即为 `{ "hero": [...] }`）
#[derive(serde::Deserialize)]
struct TencentHeroList {
    #[serde(default)]
    hero: Vec<TencentHeroEntry>,
}

#[derive(serde::Deserialize)]
struct TencentHeroEntry {
    #[serde(rename = "heroId", alias = "heroid", deserialize_with = "de_hero_id")]
    hero_id: i32,
    #[serde(default)]
    keywords: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    alias: String,
}

/// 腾讯库 heroId 为字符串数字（如 "1"），兼容数字与字符串两种形态
fn de_hero_id<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde::Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => Ok(n.as_i64().unwrap_or(0) as i32),
        serde_json::Value::String(s) => s.trim().parse::<i32>().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom("heroId 应为数字或数字字符串")),
    }
}

/// 解析英雄条目为检索串（对齐 Vue：keywords + title + alias 逗号拼接），并补充手动特例
fn parse_tencent_heroes(payload: &TencentHeroList) -> HashMap<i32, String> {
    let mut map = HashMap::new();
    for hero in &payload.hero {
        if hero.hero_id <= 0 {
            continue;
        }
        let entry = format!("{},{},{}", hero.keywords, hero.title, hero.alias);
        map.insert(hero.hero_id, entry);
    }
    // Vue championCache.ts 的手动补充特例（腾讯库缺失的新英雄/别称）
    append_extra_keyword(&mut map, 901, "小火龙");
    append_extra_keyword(&mut map, 950, "狗,那亚菲利");
    append_extra_keyword(&mut map, 902, "丁真,米利欧");
    append_extra_keyword(&mut map, 897, "黑龙,奎桑提");
    map
}

fn append_extra_keyword(map: &mut HashMap<i32, String>, id: i32, extra: &str) {
    let entry = map.entry(id).or_default();
    if entry.is_empty() {
        *entry = extra.to_string();
    } else {
        entry.push(',');
        entry.push_str(extra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tencent_heroes_joins_fields_and_appends_extras() {
        let payload: TencentHeroList = serde_json::from_str(
            r#"{"code":0,"hero":[
                {"heroId":"157","keywords":"yasuo,yss","title":"疾风剑豪","alias":"Yasuo"},
                {"heroId":902,"keywords":"milio","title":".Redis 缇米朵","alias":"Milio"},
                {"heroId":"0","keywords":"skip","title":"","alias":""}
            ]}"#,
        )
        .unwrap();

        let map = parse_tencent_heroes(&payload);
        assert_eq!(
            map.get(&157).map(String::as_str),
            Some("yasuo,yss,疾风剑豪,Yasuo")
        );
        // 902 追加手动特例（丁真/米利欧）
        let milio = map.get(&902).unwrap();
        assert!(milio.contains("丁真,米利欧"), "特例应追加: {milio}");
        // 非法 ID 跳过
        assert!(!map.contains_key(&0));
        // 特例英雄即使腾讯库缺失也有检索串
        assert!(map.get(&901).unwrap().contains("小火龙"));
    }

    #[test]
    fn keyword_lookup_defaults_to_empty_without_cache() {
        assert_eq!(champion_keywords(999999), "");
        assert_eq!(spell_icon_path(999999), None);
    }
}
