use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

#[allow(dead_code)]
pub struct TrayManager {
    _tray_icon: TrayIcon,
    pub show_item_id: MenuId,
    pub quit_item_id: MenuId,
}

#[allow(dead_code)]
impl TrayManager {
    pub fn new() -> Result<Self, String> {
        let tray_menu = Menu::new();
        let show_item = MenuItem::new("打开 Yuumi", true, None);
        let quit_item = MenuItem::new("退出", true, None);

        let show_item_id = show_item.id().clone();
        let quit_item_id = quit_item.id().clone();

        tray_menu.append(&show_item).map_err(|e| e.to_string())?;
        tray_menu.append(&quit_item).map_err(|e| e.to_string())?;

        // 动态生成 32x32 极光青色圆点图标（RGBA）
        let width = 32;
        let height = 32;
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - 16.0;
                let dy = y as f32 - 16.0;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= 12.0 {
                    // 极光青色 #00f5d4 -> r:0, g:245, b:212
                    rgba.extend_from_slice(&[0, 245, 212, 255]);
                } else if dist <= 14.0 {
                    // 边缘抗锯齿与光晕
                    let alpha = ((14.0 - dist) / 2.0 * 255.0) as u8;
                    rgba.extend_from_slice(&[0, 187, 249, alpha]);
                } else {
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }

        let icon = Icon::from_rgba(rgba, width, height).map_err(|e| e.to_string())?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Yuumi - 英雄联盟对局助手 (GPUI 原型)")
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            _tray_icon: tray_icon,
            show_item_id,
            quit_item_id,
        })
    }

    /// 检查托盘事件（菜单点击或图标点击）
    pub fn poll_event(&self) -> Option<TrayAction> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.show_item_id {
                return Some(TrayAction::ShowWindow);
            } else if event.id == self.quit_item_id {
                return Some(TrayAction::Quit);
            }
        }

        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::Click { button, .. } => {
                    if button == tray_icon::MouseButton::Left {
                        return Some(TrayAction::ToggleWindow);
                    }
                }
                TrayIconEvent::DoubleClick { .. } => {
                    return Some(TrayAction::ShowWindow);
                }
                _ => {}
            }
        }

        None
    }
}

#[allow(dead_code)]
pub enum TrayAction {
    ShowWindow,
    ToggleWindow,
    Quit,
}
