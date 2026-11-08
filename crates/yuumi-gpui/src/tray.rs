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

        // 复用 exe 内嵌的 icon.ico（资源 ID 1，含多尺寸），与窗口任务栏图标同源
        let icon = Icon::from_resource(1, None).map_err(|e| e.to_string())?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Yuumi")
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
