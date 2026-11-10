//! OP.GG 组件共享的格式化与图标路径解析
//!
//! 对照 Vue `src/components/opgg/shared.ts`：百分比一位小数、KDA 两位小数，
//! 图标优先取 LCU 预加载的资源映射，缺失时退回标准 LCU 资源路径。

use crate::opgg::RuneCatalog;
use yuumi_core::lcu::game_data::GameDataAssets;

/// 百分比格式化：0.5234 → "52.3%"，缺失显示短横线
pub fn pct(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{:.1}%", v * 100.0),
        None => "-".to_string(),
    }
}

/// KDA 格式化：保留两位小数，缺失显示短横线
pub fn fmt_kda(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{v:.2}"),
        None => "-".to_string(),
    }
}

/// 场次文案（对照 `career.gamesCount` → "{count} 场"）
pub fn games_count(play: u32) -> String {
    format!("{play} 场")
}

/// 英雄头像路径
pub fn champion_icon(champion_id: i32) -> String {
    format!("/lol-game-data/assets/v1/champion-icons/{champion_id}.png")
}

/// 装备图标：优先预加载映射，兜底 LCU 路径
pub fn item_icon(assets: Option<&GameDataAssets>, item_id: i32) -> String {
    if let Some(path) = assets.and_then(|assets| assets.items.get(&item_id)) {
        return path.clone();
    }
    format!("/lol-game-data/assets/v1/items/{item_id}.png")
}

/// 召唤师技能图标：优先预加载映射，兜底 LCU 路径
pub fn spell_icon(assets: Option<&GameDataAssets>, spell_id: i32) -> String {
    if let Some(path) = assets.and_then(|assets| assets.spells.get(&spell_id)) {
        return path.clone();
    }
    format!("/lol-game-data/assets/v1/summoner-spells/{spell_id}.png")
}

/// 符文图标：优先预加载映射，其次符文体系图标，兜底 LCU 路径
pub fn rune_icon(assets: Option<&GameDataAssets>, catalog: &RuneCatalog, rune_id: i32) -> String {
    if let Some(path) = assets.and_then(|assets| assets.runes.get(&rune_id)) {
        return path.clone();
    }
    if let Some(style) = catalog.style(rune_id) {
        if !style.icon_path.is_empty() {
            return style.icon_path.clone();
        }
    }
    format!("/lol-game-data/assets/v1/perks/{rune_id}.png")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct_keeps_one_decimal_and_dashes_missing() {
        assert_eq!(pct(Some(0.5234)), "52.3%");
        assert_eq!(pct(Some(0.5)), "50.0%");
        assert_eq!(pct(None), "-");
    }

    #[test]
    fn kda_keeps_two_decimals() {
        assert_eq!(fmt_kda(Some(2.8)), "2.80");
        assert_eq!(fmt_kda(Some(4.0)), "4.00");
        assert_eq!(fmt_kda(None), "-");
    }

    #[test]
    fn icon_resolution_falls_back_to_lcu_path() {
        let assets = GameDataAssets::default();
        assert_eq!(
            item_icon(Some(&assets), 6655),
            "/lol-game-data/assets/v1/items/6655.png"
        );
        assert_eq!(
            spell_icon(Some(&assets), 4),
            "/lol-game-data/assets/v1/summoner-spells/4.png"
        );
        let catalog = RuneCatalog::default();
        assert_eq!(
            rune_icon(Some(&assets), &catalog, 8214),
            "/lol-game-data/assets/v1/perks/8214.png"
        );
    }

    #[test]
    fn icon_resolution_prefers_preloaded_mapping() {
        let mut assets = GameDataAssets::default();
        assets
            .items
            .insert(6655, "/preloaded/items/6655.png".into());
        assets.spells.insert(4, "/preloaded/spells/4.png".into());
        assets.runes.insert(8214, "/preloaded/runes/8214.png".into());
        assert_eq!(item_icon(Some(&assets), 6655), "/preloaded/items/6655.png");
        assert_eq!(spell_icon(Some(&assets), 4), "/preloaded/spells/4.png");
        assert_eq!(
            rune_icon(Some(&assets), &RuneCatalog::default(), 8214),
            "/preloaded/runes/8214.png"
        );
    }

    #[test]
    fn rune_icon_uses_style_icon_when_asset_missing() {
        let catalog = RuneCatalog {
            styles: vec![crate::opgg::PerkStyle {
                id: 8000,
                icon_path: "/lol-game-data/assets/v1/perkstyles/8000.png".into(),
                slots: Vec::new(),
            }],
            perks: Default::default(),
        };
        assert_eq!(
            rune_icon(None, &catalog, 8000),
            "/lol-game-data/assets/v1/perkstyles/8000.png"
        );
    }
}
