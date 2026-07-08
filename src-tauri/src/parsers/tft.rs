use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

use crate::{build_auth_header, AppState};

// ─── TFT 数据结构 ───

/// TFT 解析后的资源映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TftDataMapping {
    pub champions: HashMap<String, String>,
    pub traits: HashMap<String, String>,
    pub champion_icons: HashMap<String, String>,
    pub item_icons: HashMap<String, String>,
    pub item_names: HashMap<String, String>,
}

// ─── LCU 原始 JSON 结构 ───

#[derive(Debug, Clone, Deserialize)]
struct TftJsonRoot {
    sets: Option<HashMap<String, TftSet>>,
}

#[derive(Debug, Clone, Deserialize)]
struct TftSet {
    champions: Option<Vec<TftChampion>>,
    traits: Option<Vec<TftTrait>>,
}

#[derive(Debug, Clone, Deserialize)]
struct TftChampion {
    #[serde(rename = "apiName")]
    api_name: Option<String>,
    name: Option<String>,
    #[serde(rename = "squareIcon")]
    square_icon: Option<String>,
    icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TftTrait {
    #[serde(rename = "apiName")]
    api_name: Option<String>,
    name: Option<String>,
    icon: Option<String>,
}

// ─── 解析逻辑 ───

/// 从 TFT JSON 内容解析资源映射（对应 Python `parseData`）
fn parse_tft_data(content: &TftJsonRoot) -> TftDataMapping {
    let mut mapping = TftDataMapping {
        champions: HashMap::new(),
        traits: HashMap::new(),
        champion_icons: HashMap::new(),
        item_icons: HashMap::new(),
        item_names: HashMap::new(),
    };

    if let Some(sets) = &content.sets {
        for set_data in sets.values() {
            if let Some(champs) = &set_data.champions {
                for champ in champs {
                    if let Some(ref api_name) = champ.api_name {
                        let name = champ.name.clone().unwrap_or_default();
                        let icon = champ
                            .square_icon
                            .clone()
                            .or_else(|| champ.icon.clone())
                            .unwrap_or_default();
                        mapping.champions.insert(api_name.clone(), name);
                        mapping.champion_icons.insert(api_name.clone(), icon);
                    }
                }
            }
            if let Some(traits) = &set_data.traits {
                for trait_data in traits {
                    if let Some(ref api_name) = trait_data.api_name {
                        let name = trait_data.name.clone().unwrap_or_default();
                        let _icon = trait_data.icon.clone().unwrap_or_default();
                        mapping.traits.insert(api_name.clone(), name);
                    }
                }
            }
        }
    }

    mapping
}

// ─── Tauri 命令 ───

/// 从 LCU 获取 TFT 数据资源
#[tauri::command]
pub async fn get_tft_data(app_state: State<'_, AppState>) -> Result<TftDataMapping, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let auth = build_auth_header(&lcu.token);

    // 优先从 LCU 资源池获取
    let lcu_url = format!(
        "https://127.0.0.1:{}/lol-game-data/assets/v1/tft.json",
        lcu.port
    );

    match lcu
        .http_client
        .get(&lcu_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let content: TftJsonRoot = resp.json().await.map_err(|e| e.to_string())?;
            return Ok(parse_tft_data(&content));
        }
        _ => {
            log::info!("LCU TFT 数据不可用，尝试从 CDragon 获取");
        }
    }

    // 备用：从 Community Dragon CDN 获取
    let cdn_url = "https://raw.communitydragon.org/latest/cdragon/tft/zh_cn.json";

    let http = reqwest::Client::new();
    let resp = http
        .get(cdn_url)
        .send()
        .await
        .map_err(|e| format!("CDragon 请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("CDragon 返回错误: HTTP {}", resp.status()));
    }

    let content: TftJsonRoot = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parse_tft_data(&content))
}
