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
    /// 路由标题（经 i18n 查表，侧栏 / 托盘 / 页面共用）
    pub fn title(&self) -> &'static str {
        match self {
            Route::Home => crate::i18n::t("nav.home"),
            Route::Career => crate::i18n::t("nav.career"),
            Route::Search => crate::i18n::t("nav.search"),
            Route::GameInfo => crate::i18n::t("nav.gameinfo"),
            Route::Tft => crate::i18n::t("nav.tft"),
            Route::SavedPlayers => crate::i18n::t("nav.savedPlayers"),
            Route::Tools => crate::i18n::t("nav.tools"),
            Route::Settings => crate::i18n::t("nav.settings"),
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
