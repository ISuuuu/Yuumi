//! 自动化 Agent 调度与任务模块（生命周期、独占并发管理、BP与匹配）

pub mod auto_bp;
pub mod auto_match;
pub mod config;
pub mod lifecycle;
pub mod supervisor;

pub use auto_bp::{handle_champ_select_session, ChampSelectSession};
pub use auto_match::{handle_phase_change, handle_ready_check, ReadyCheckData};
pub use config::{AutoBpConfig, AutoMatchConfig};
pub use lifecycle::{GamePhase, GenerationGuard, LifecycleManager, LifecycleState, PhaseTransition};
pub use supervisor::{AgentSupervisor, DuplicateStrategy, SpawnError, TaskId, TaskKind};
