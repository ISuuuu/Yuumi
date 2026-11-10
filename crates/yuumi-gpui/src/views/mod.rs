pub mod career;
pub mod game_info;
pub mod home;
pub mod saved_players;
pub mod search;
pub mod settings;
pub mod tft;
pub mod tools;

pub use career::CareerView;
pub use game_info::{GameInfoTeamTab, GameInfoView, GameInfoViewMode};
pub use home::HomeView;
pub use saved_players::SavedPlayersView;
pub use search::SearchView;
pub use settings::SettingsView;
pub use tft::{TftAction, TftTab, TftView};
pub use tools::ToolsView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Home,
    Career,
    Search,
    GameInfo,
    Tft,
    SavedPlayers,
    Tools,
    Settings,
}

impl Route {
    pub fn title(&self) -> &'static str {
        match self {
            Route::Home => "首页",
            Route::Career => "生涯",
            Route::Search => "战绩查询",
            Route::GameInfo => "对局信息",
            Route::Tft => "云顶之弈",
            Route::SavedPlayers => "路人集",
            Route::Tools => "工具箱",
            Route::Settings => "设置",
        }
    }

    /// 侧栏线性图标资源路径（`crate::assets::Assets` 登记的内嵌 SVG）
    pub fn icon_path(&self) -> &'static str {
        match self {
            Route::Home => "icons/home.svg",
            Route::Career => "icons/user.svg",
            Route::Search => "icons/search.svg",
            Route::GameInfo => "icons/gamepad.svg",
            Route::Tft => "icons/tft.svg",
            Route::SavedPlayers => "icons/bookmark.svg",
            Route::Tools => "icons/wrench.svg",
            Route::Settings => "icons/settings.svg",
        }
    }
}
