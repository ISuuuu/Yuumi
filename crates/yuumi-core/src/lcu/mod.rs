pub mod asset_cache;
pub mod client;
pub mod event_projection;
pub mod game_data;
pub mod monitor;
pub mod ws;

pub use asset_cache::AssetCache;
pub use client::{build_auth_header, LcuClient};
pub use event_projection::{project, EventDedup, WsEventPayload};
pub use game_data::{fetch_game_data_assets, CherryAugmentDetail, GameDataAssets};
pub use monitor::{detect_lcu, start_monitor, LcuConnectionInfo, MonitorEvent};
pub use ws::{start_ws, LcuWsEvent, LcuWsHandle, WsLifecycleEvent, WATCHED_URIS};
