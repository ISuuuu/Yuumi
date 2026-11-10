use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LiveEvent {
    #[serde(rename = "EventID")]
    event_id: i32,
    event_name: String,
    killer_name: Option<String>,
    legendary_to: Option<String>,
    recipient: Option<String>,
    kill_streak: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LiveEvents {
    events: Vec<LiveEvent>,
}

#[derive(Default)]
pub struct ScreenshotMonitor {
    last_id: Option<i32>,
}

impl ScreenshotMonitor {
    pub fn reset(&mut self) {
        self.last_id = None;
    }

    pub fn process(
        &mut self,
        json: &str,
        player: &str,
        enabled: bool,
        levels: &[u32],
    ) -> Result<Vec<String>, String> {
        let mut events = serde_json::from_str::<LiveEvents>(json)
            .map_err(|e| e.to_string())?
            .events;
        events.sort_by_key(|event| event.event_id);
        if self.last_id.is_none() {
            self.last_id = events.last().map(|event| event.event_id);
            return Ok(Vec::new());
        }
        let mut reasons = Vec::new();
        for event in events {
            if event.event_id <= self.last_id.unwrap_or(-1) {
                continue;
            }
            self.last_id = Some(event.event_id);
            if !enabled {
                continue;
            }
            let reason = match event.event_name.as_str() {
                "Multikill"
                    if event
                        .killer_name
                        .as_deref()
                        .is_some_and(|name| same_player(name, player)) =>
                {
                    event
                        .kill_streak
                        .filter(|level| levels.contains(level))
                        .map(|level| match level {
                            3 => "TripleKill".to_string(),
                            4 => "QuadraKill".to_string(),
                            5 => "PentaKill".to_string(),
                            _ => format!("Multikill_{level}"),
                        })
                }
                "Legendary"
                    if levels.contains(&8)
                        && event
                            .legendary_to
                            .as_deref()
                            .or(event.killer_name.as_deref())
                            .or(event.recipient.as_deref())
                            .is_some_and(|name| same_player(name, player)) =>
                {
                    Some("Legendary".to_string())
                }
                _ => None,
            };
            if let Some(reason) = reason {
                reasons.push(reason);
            }
        }
        Ok(reasons)
    }
}

fn same_player(a: &str, b: &str) -> bool {
    if a.contains('#') && b.contains('#') {
        return a.trim().to_lowercase() == b.trim().to_lowercase();
    }
    a.split('#').next().unwrap_or(a).trim().to_lowercase()
        == b.split('#').next().unwrap_or(b).trim().to_lowercase()
}

pub fn screenshot_dir(custom_path: &str) -> Result<PathBuf, String> {
    if !custom_path.is_empty() {
        return Ok(PathBuf::from(custom_path));
    }
    dirs::picture_dir()
        .map(|p| p.join("Yuumi_Screenshots"))
        .ok_or_else(|| "无法定位系统图片文件夹".to_string())
}

/// Blocking operation: invoke on a background thread.
pub fn capture(player: &str, reason: &str, custom_path: &str) -> Result<PathBuf, String> {
    let dir = screenshot_dir(custom_path)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建截图目录失败: {e}"))?;
    let safe_player: String = player
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let path = dir.join(format!(
        "{}_{}_{}.png",
        reason,
        safe_player,
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    save_capture(&path)?;
    Ok(path)
}

fn save_capture(path: &Path) -> Result<(), String> {
    use xcap::{Monitor, Window};
    let windows = Window::all().map_err(|e| e.to_string())?;
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let monitor = windows
        .iter()
        .find(|w| w.title() == "League of Legends (TM) Client")
        .and_then(|w| {
            let x = w.x() + w.width() as i32 / 2;
            let y = w.y() + w.height() as i32 / 2;
            monitors.iter().find(|m| {
                x >= m.x()
                    && x < m.x() + m.width() as i32
                    && y >= m.y()
                    && y < m.y() + m.height() as i32
            })
        })
        .or_else(|| monitors.first())
        .ok_or_else(|| "未检测到任何可用显示器".to_string())?;
    monitor
        .capture_image()
        .map_err(|e| e.to_string())?
        .save(path)
        .map_err(|e| format!("写入截图失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_with_two_tags_require_the_same_tag() {
        assert!(same_player("Player#ABC", "player#abc"));
        assert!(!same_player("Player#ABC", "player#other"));
        assert!(same_player("Player#ABC", "player"));
    }
    #[test]
    fn events_initialize_filter_and_deduplicate() {
        let mut monitor = ScreenshotMonitor::default();
        let old = r#"{"Events":[{"EventID":1,"EventName":"Multikill","KillerName":"test","KillStreak":3}]}"#;
        let new = r#"{"Events":[{"EventID":3,"EventName":"Legendary","Recipient":"TEST#1"},{"EventID":2,"EventName":"Multikill","KillerName":"other","KillStreak":3},{"EventID":4,"EventName":"Multikill","KillerName":"test","KillStreak":3}]}"#;
        assert!(monitor
            .process(old, "test", true, &[3, 8])
            .unwrap()
            .is_empty());
        assert_eq!(
            monitor.process(new, "test", true, &[3, 8]).unwrap(),
            vec!["Legendary", "TripleKill"]
        );
        assert!(monitor
            .process(new, "test", true, &[3, 8])
            .unwrap()
            .is_empty());
        monitor.reset();
        assert!(monitor
            .process(new, "test", true, &[3, 8])
            .unwrap()
            .is_empty());
        assert!(monitor.process("invalid", "test", true, &[]).is_err());
    }

    #[test]
    fn invalid_destination_fails_before_capture() {
        let path =
            std::env::temp_dir().join(format!("yuumi-screenshot-test-{}", std::process::id()));
        std::fs::write(&path, b"synthetic").expect("create fixture file");
        let result = capture("test", "TripleKill", &path.to_string_lossy());
        assert!(result.is_err());
        std::fs::remove_file(path).expect("remove fixture file");
    }
}
