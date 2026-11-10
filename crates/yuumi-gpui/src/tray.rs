use crate::views::Route;
use std::collections::HashMap;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    ShowWindow,
    ToggleWindow,
    Navigate(Route),
    Quit,
}

pub struct TrayManager {
    _tray_icon: TrayIcon,
    menu_actions: HashMap<MenuId, TrayAction>,
}

impl TrayManager {
    pub fn new(hide_tft: bool) -> Result<Self, String> {
        let (tray_menu, menu_actions) = Self::build_menu(hide_tft)?;

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
            menu_actions,
        })
    }

    /// 构建系统托盘菜单（对齐 Vue build_tray_menu 结构）
    fn build_menu(hide_tft: bool) -> Result<(Menu, HashMap<MenuId, TrayAction>), String> {
        let tray_menu = Menu::new();
        let mut menu_actions = HashMap::new();

        // 1. 主页
        let item_home = MenuItem::new("主页", true, None);
        menu_actions.insert(item_home.id().clone(), TrayAction::Navigate(Route::Home));
        tray_menu.append(&item_home).map_err(|e| e.to_string())?;

        // 2. 生涯
        let item_career = MenuItem::new("生涯", true, None);
        menu_actions.insert(item_career.id().clone(), TrayAction::Navigate(Route::Career));
        tray_menu.append(&item_career).map_err(|e| e.to_string())?;

        // 3. 战绩查询
        let item_search = MenuItem::new("战绩查询", true, None);
        menu_actions.insert(item_search.id().clone(), TrayAction::Navigate(Route::Search));
        tray_menu.append(&item_search).map_err(|e| e.to_string())?;

        // 4. 对局信息
        let item_gameinfo = MenuItem::new("对局信息", true, None);
        menu_actions.insert(item_gameinfo.id().clone(), TrayAction::Navigate(Route::GameInfo));
        tray_menu.append(&item_gameinfo).map_err(|e| e.to_string())?;

        // 5. TFT (若未隐藏)
        if !hide_tft {
            let item_tft = MenuItem::new("TFT", true, None);
            menu_actions.insert(item_tft.id().clone(), TrayAction::Navigate(Route::Tft));
            tray_menu.append(&item_tft).map_err(|e| e.to_string())?;
        }

        // 6. 其他功能
        let item_tools = MenuItem::new("其他功能", true, None);
        menu_actions.insert(item_tools.id().clone(), TrayAction::Navigate(Route::Tools));
        tray_menu.append(&item_tools).map_err(|e| e.to_string())?;

        // 分隔线
        let sep1 = PredefinedMenuItem::separator();
        tray_menu.append(&sep1).map_err(|e| e.to_string())?;

        // 7. 设置
        let item_settings = MenuItem::new("设置", true, None);
        menu_actions.insert(item_settings.id().clone(), TrayAction::Navigate(Route::Settings));
        tray_menu.append(&item_settings).map_err(|e| e.to_string())?;

        // 分隔线
        let sep2 = PredefinedMenuItem::separator();
        tray_menu.append(&sep2).map_err(|e| e.to_string())?;

        // 8. 退出
        let item_quit = MenuItem::new("退出", true, None);
        menu_actions.insert(item_quit.id().clone(), TrayAction::Quit);
        tray_menu.append(&item_quit).map_err(|e| e.to_string())?;

        Ok((tray_menu, menu_actions))
    }

    /// 动态更新托盘菜单（如配置变化时）
    pub fn update_menu(&mut self, hide_tft: bool) -> Result<(), String> {
        let (tray_menu, menu_actions) = Self::build_menu(hide_tft)?;
        self._tray_icon.set_menu(Some(Box::new(tray_menu)));
        self.menu_actions = menu_actions;
        Ok(())
    }

    /// 检查托盘事件（菜单点击或图标点击）
    pub fn poll_event(&self) -> Option<TrayAction> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(action) = self.menu_actions.get(&event.id) {
                return Some(action.clone());
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
