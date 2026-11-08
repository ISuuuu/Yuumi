pub mod career;
pub mod game_info;
pub mod home;
pub mod saved_players;
pub mod search;
pub mod settings;
pub mod tools;

pub use career::CareerView;
pub use game_info::GameInfoView;
pub use home::HomeView;
pub use saved_players::SavedPlayersView;
pub use search::SearchView;
pub use settings::SettingsView;
pub use tools::ToolsView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Home,
    Career,
    Search,
    GameInfo,
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
            Route::SavedPlayers => "路人集",
            Route::Tools => "工具箱",
            Route::Settings => "设置",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Route::Home => "🏠",
            Route::Career => "📊",
            Route::Search => "🔍",
            Route::GameInfo => "⚔️",
            Route::SavedPlayers => "🔖",
            Route::Tools => "🧰",
            Route::Settings => "⚙️",
        }
    }
}
