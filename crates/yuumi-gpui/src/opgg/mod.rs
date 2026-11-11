//! OP.GG 独立窗口模块
//!
//! 对照 Vue 版的独立窗口入口 `src/components/OpggWindow.vue` 与页面主体
//! `src/components/OpggModal.vue`，渲染组件按 `src/components/opgg/` 一一对应拆分：
//! `filter_bar` / `tier_list` / `build_detail` / `runes` / `skill_order` / `items`。
//!
//! 与 Vue 的差异仅在传输层：Vue 经 Tauri `invoke("fetch_opgg_data")` 取数，
//! 这里直接调用 `yuumi_core::opgg`，少一次 IPC 序列化。

pub mod build_detail;
pub mod filter_bar;
pub mod format;
pub mod items;
pub mod runes;
pub mod skill_order;
pub mod tier_list;
pub mod window;

pub use window::{OpggWindow, OPGG_WINDOW_TITLE};

use gpui::{rgb, rgba, Hsla};
use serde::Deserialize;
use std::collections::HashMap;
use yuumi_core::lcu::client::LcuClient;

// ─── 局部配色 ───

/// OP.GG 窗口专用配色。
///
/// 除这里列出的 OP.GG 特有色值外，其余全部复用 `Theme` 的语义色，
/// 与 Vue 侧「优先使用全局变量、个别硬编码」的写法保持一致。
pub struct Palette;

impl Palette {
    /// 详情页软灰背景，对照 `.build-view-bg`
    pub fn build_bg() -> Hsla {
        rgb(0xf1f3f7).into()
    }

    /// 头部 OP.GG 标识色，对照 `.opgg-logo`
    pub fn logo() -> Hsla {
        rgb(0x636ff9).into()
    }

    /// 激活页签底色，对照 `.tab-btn.active`
    pub fn tab_active() -> Hsla {
        rgb(0x2fbc5d).into()
    }

    /// 激活页签悬停底色，对照 `.tab-btn.active:hover`
    pub fn tab_active_hover() -> Hsla {
        rgb(0x25a44e).into()
    }

    /// 符文选中描边（琥珀）
    pub fn rune_active() -> Hsla {
        rgb(0xe5a93b).into()
    }

    /// 属性碎片选中描边（亮绿）
    pub fn shard_active() -> Hsla {
        rgb(0x51cf66).into()
    }

    /// 召唤师技能预设条底色，对照 `.spell-pair` 的 `rgba(0, 0, 0, 0.015)`
    pub fn spell_pair_bg() -> Hsla {
        rgba(0x00000004).into()
    }

    /// 符文容器底色，对照 `.rune-tree-container` 的 `rgba(0, 0, 0, 0.01)`
    pub fn rune_tree_bg() -> Hsla {
        rgba(0x00000003).into()
    }

    /// 克制关系「优势」文字色（--win-color）
    pub fn win_text() -> Hsla {
        rgb(0x10b981).into()
    }

    /// 克制关系「劣势」文字色（--loss-color）
    pub fn loss_text() -> Hsla {
        rgb(0xf43f5e).into()
    }
}

/// Tier 徽章底色，对照 `BuildDetailPanel.vue` 的 `TIER_COLORS`
pub fn tier_badge_color(tier: Option<&str>) -> Hsla {
    match tier {
        Some("0") | Some("1") => rgb(0x5b8db8).into(),
        Some("2") => rgb(0x5ba8a3).into(),
        Some("3") => rgb(0xb89b52).into(),
        _ => rgb(0x7d8185).into(),
    }
}

/// 榜单卡片背景，对照 `TierListPanel.vue` 的 `TIER_CARD_BG`（亮色表）
pub fn tier_card_bg(tier: Option<&str>) -> Hsla {
    match tier {
        Some("0") | Some("1") => rgb(0xcde5f8).into(),
        Some("2") => rgb(0xcdecea).into(),
        Some("3") => rgb(0xf4ead1).into(),
        Some("4") | Some("5") => rgb(0xe5e8ec).into(),
        _ => rgba(0x00000000).into(),
    }
}

/// 榜单卡片描边，对照 `TIER_CARD_BORDER`（亮色表统一为 `rgba(0, 0, 0, 0.095)`）
pub fn tier_card_border() -> Hsla {
    rgba(0x00000018).into()
}

/// 详情标题栏背景，对照 `BuildDetailPanel.vue` 的 `TIER_BG_COLORS`
pub fn tier_title_bg(tier: Option<&str>) -> Hsla {
    match tier {
        Some("0") | Some("1") => rgb(0xcde5f8).into(),
        Some("2") => rgb(0xcdecea).into(),
        Some("3") => rgb(0xf4ead1).into(),
        Some("4") | Some("5") => rgb(0xe5e8ec).into(),
        _ => rgb(0xf8f9fa).into(),
    }
}

/// 详情标题栏描边，对照 `TIER_BORDER_COLORS`（亮色表统一为 `rgba(0, 0, 0, 0.095)`）
pub fn tier_title_border() -> Hsla {
    rgba(0x00000018).into()
}

// ─── LCU 符文静态资源 ───

/// 符文树的一个插槽
#[derive(Debug, Clone, Default)]
pub struct PerkSlot {
    pub perks: Vec<i32>,
}

/// 符文体系（`perkstyles.json`）
#[derive(Debug, Clone, Default)]
pub struct PerkStyle {
    pub id: i32,
    pub icon_path: String,
    pub slots: Vec<PerkSlot>,
}

/// 单个符文的名称与说明（`perks.json`）
#[derive(Debug, Clone, Default)]
pub struct PerkInfo {
    pub name: String,
    pub short_desc: String,
}

/// 符文树渲染所需的静态资料
#[derive(Debug, Clone, Default)]
pub struct RuneCatalog {
    pub styles: Vec<PerkStyle>,
    pub perks: HashMap<i32, PerkInfo>,
}

impl RuneCatalog {
    pub fn style(&self, style_id: i32) -> Option<&PerkStyle> {
        self.styles.iter().find(|style| style.id == style_id)
    }

    /// 符文悬停提示：名称 + 清理 HTML 后的说明（对照 Vue `perkTitle`）
    pub fn tooltip(&self, perk_id: i32) -> String {
        match self.perks.get(&perk_id) {
            Some(perk) if perk.short_desc.is_empty() => perk.name.clone(),
            Some(perk) => format!("{}\n{}", perk.name, perk.short_desc),
            None => String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPerkStyles {
    List(Vec<RawPerkStyle>),
    Wrapped {
        #[serde(default)]
        styles: Vec<RawPerkStyle>,
    },
}

#[derive(Debug, Deserialize)]
struct RawPerkStyle {
    #[serde(default)]
    id: i32,
    #[serde(rename = "iconPath", default)]
    icon_path: String,
    #[serde(default)]
    slots: Vec<RawPerkSlot>,
}

#[derive(Debug, Deserialize)]
struct RawPerkSlot {
    #[serde(default)]
    perks: Vec<i32>,
}

#[derive(Debug, Deserialize)]
struct RawPerk {
    #[serde(default)]
    id: i32,
    #[serde(default)]
    name: String,
    #[serde(rename = "shortDesc", default)]
    short_desc: String,
}

/// 去除 HTML 标签与常见实体（对照 Vue `cleanDescription`）
pub fn clean_description(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(ch),
            _ => {}
        }
    }
    plain
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}

/// 从 LCU 读取符文树结构与符文文案。
///
/// 两份资料独立于统计请求加载：任一失败只降级为「无符文图标/无提示」，不阻塞正文。
pub async fn load_rune_catalog(client: &LcuClient) -> RuneCatalog {
    let styles = match client.get("/lol-game-data/assets/v1/perkstyles.json").await {
        Ok(value) => match serde_json::from_value::<RawPerkStyles>(value) {
            Ok(RawPerkStyles::List(list)) => list,
            Ok(RawPerkStyles::Wrapped { styles }) => styles,
            Err(err) => {
                log::warn!("[OPGG] 解析 perkstyles.json 失败: {err}");
                Vec::new()
            }
        }
        .into_iter()
        .map(|style| PerkStyle {
            id: style.id,
            icon_path: style.icon_path,
            slots: style
                .slots
                .into_iter()
                .map(|slot| PerkSlot { perks: slot.perks })
                .collect(),
        })
        .collect(),
        Err(err) => {
            log::warn!("[OPGG] 读取 perkstyles.json 失败: {err}");
            Vec::new()
        }
    };

    let perks = match client.get("/lol-game-data/assets/v1/perks.json").await {
        Ok(value) => match serde_json::from_value::<Vec<RawPerk>>(value) {
            Ok(list) => list
                .into_iter()
                .map(|perk| {
                    (
                        perk.id,
                        PerkInfo {
                            name: perk.name,
                            short_desc: clean_description(&perk.short_desc),
                        },
                    )
                })
                .collect(),
            Err(err) => {
                log::warn!("[OPGG] 解析 perks.json 失败: {err}");
                HashMap::new()
            }
        },
        Err(err) => {
            log::warn!("[OPGG] 读取 perks.json 失败: {err}");
            HashMap::new()
        }
    };

    log::info!(
        "[OPGG] 符文资料就绪: 体系 {} 个, 符文 {} 条",
        styles.len(),
        perks.len()
    );
    RuneCatalog { styles, perks }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_description_strips_tags_and_entities() {
        assert_eq!(
            clean_description("<b>回复</b> 6%&nbsp;法力值 &amp; 生命"),
            "回复 6% 法力值 & 生命"
        );
        assert_eq!(clean_description(""), "");
    }

    #[test]
    fn perk_styles_accept_wrapped_and_list_payloads() {
        let wrapped: RawPerkStyles = serde_json::from_str(
            r#"{"styles":[{"id":8000,"name":"精密","iconPath":"/a.png","slots":[{"perks":[8005,8009]}]}]}"#,
        )
        .expect("对象载荷应可解析");
        let list: RawPerkStyles =
            serde_json::from_str(r#"[{"id":8100,"name":"主宰","iconPath":"/b.png","slots":[]}]"#)
                .expect("数组载荷应可解析");
        match wrapped {
            RawPerkStyles::Wrapped { styles } => assert_eq!(styles.len(), 1),
            RawPerkStyles::List(_) => panic!("对象载荷不应解析为数组"),
        }
        match list {
            RawPerkStyles::List(styles) => assert_eq!(styles[0].id, 8100),
            RawPerkStyles::Wrapped { .. } => panic!("数组载荷不应解析为对象"),
        }
    }

    #[test]
    fn catalog_tooltip_joins_name_and_description() {
        let mut catalog = RuneCatalog::default();
        catalog.perks.insert(
            8005,
            PerkInfo {
                name: "强攻".into(),
                short_desc: "连续攻击同一目标".into(),
            },
        );
        assert_eq!(catalog.tooltip(8005), "强攻\n连续攻击同一目标");
        assert_eq!(catalog.tooltip(1), "");
    }
}
