//! 应用后台服务模块

pub mod agent_runtime;
pub mod champion_alias;
pub mod challenges;
pub mod cloud_tasks;
pub mod friend_cleaner;
pub mod live_lcu;
pub mod logging;
pub mod profile_background;
pub mod screenshot;
pub mod spectate;
pub mod system_tools;

pub use agent_runtime::{AgentRuntime, BpAgentEvent, MatchAgentEvent};
pub use live_lcu::{ConnectionStatus, LiveLcuEvent, LiveLcuService};
pub use system_tools::*;
