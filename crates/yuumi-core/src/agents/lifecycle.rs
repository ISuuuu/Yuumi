//! 自动化 Agent 生命周期代际管理（下沉自原 src-tauri/src/agents/lifecycle.rs）

use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// 游戏流程阶段枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamePhase {
    None,
    Lobby,
    Matchmaking,
    ReadyCheck,
    ChampSelect,
    GameStart,
    InProgress,
    PreEndOfGame,
    WaitingForStats,
    EndOfGame,
    Reconnect,
    WatchInProgress,
    Unknown(String),
}

impl From<&str> for GamePhase {
    fn from(s: &str) -> Self {
        match s {
            "None" => GamePhase::None,
            "Lobby" => GamePhase::Lobby,
            "Matchmaking" => GamePhase::Matchmaking,
            "ReadyCheck" => GamePhase::ReadyCheck,
            "ChampSelect" => GamePhase::ChampSelect,
            "GameStart" => GamePhase::GameStart,
            "InProgress" => GamePhase::InProgress,
            "PreEndOfGame" => GamePhase::PreEndOfGame,
            "WaitingForStats" => GamePhase::WaitingForStats,
            "EndOfGame" => GamePhase::EndOfGame,
            "Reconnect" => GamePhase::Reconnect,
            "WatchInProgress" => GamePhase::WatchInProgress,
            other => GamePhase::Unknown(other.to_string()),
        }
    }
}

impl GamePhase {
    pub fn as_str(&self) -> &str {
        match self {
            GamePhase::None => "None",
            GamePhase::Lobby => "Lobby",
            GamePhase::Matchmaking => "Matchmaking",
            GamePhase::ReadyCheck => "ReadyCheck",
            GamePhase::ChampSelect => "ChampSelect",
            GamePhase::GameStart => "GameStart",
            GamePhase::InProgress => "InProgress",
            GamePhase::PreEndOfGame => "PreEndOfGame",
            GamePhase::WaitingForStats => "WaitingForStats",
            GamePhase::EndOfGame => "EndOfGame",
            GamePhase::Reconnect => "Reconnect",
            GamePhase::WatchInProgress => "WatchInProgress",
            GamePhase::Unknown(s) => s.as_str(),
        }
    }

    /// 是否处于活跃对局相关的阶段 (从确认开始到游戏结算完毕)
    pub fn is_match_active(&self) -> bool {
        matches!(
            self,
            GamePhase::ReadyCheck
                | GamePhase::ChampSelect
                | GamePhase::GameStart
                | GamePhase::InProgress
                | GamePhase::PreEndOfGame
                | GamePhase::WaitingForStats
                | GamePhase::EndOfGame
        )
    }

    /// 是否处于局内加载或对局运行中
    pub fn is_in_game(&self) -> bool {
        matches!(self, GamePhase::GameStart | GamePhase::InProgress)
    }
}

/// 生命周期代际失效错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleGenerationError {
    EpochMismatch { expected: u64, actual: u64 },
    RevisionMismatch { expected: u64, actual: u64 },
    SessionMismatch { expected: String, actual: Option<String> },
    SessionMissing,
}

impl std::fmt::Display for StaleGenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StaleGenerationError::EpochMismatch { expected, actual } => {
                write!(f, "对局代际不匹配: 期望 epoch={}, 实际={}", expected, actual)
            }
            StaleGenerationError::RevisionMismatch { expected, actual } => {
                write!(f, "阶段版本不匹配: 期望 revision={}, 实际={}", expected, actual)
            }
            StaleGenerationError::SessionMismatch { expected, actual } => {
                write!(f, "选人会话不匹配: 期望 session={}, 实际={:?}", expected, actual)
            }
            StaleGenerationError::SessionMissing => {
                write!(f, "当前没有活跃的选人会话")
            }
        }
    }
}

impl std::error::Error for StaleGenerationError {}

/// 当前游戏世界权威状态快照
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleState {
    pub match_epoch: u64,
    pub phase_revision: u64,
    pub phase: GamePhase,
    pub session_id: Option<String>,
}

/// 任务代际守卫：在产生外部副作用前验证有效性
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationGuard {
    pub epoch: u64,
    pub revision: u64,
    pub required_session_id: Option<String>,
}

impl GenerationGuard {
    pub fn ensure_valid(&self, current: &LifecycleState) -> Result<(), StaleGenerationError> {
        if self.epoch != current.match_epoch {
            return Err(StaleGenerationError::EpochMismatch {
                expected: self.epoch,
                actual: current.match_epoch,
            });
        }

        if self.revision != current.phase_revision {
            return Err(StaleGenerationError::RevisionMismatch {
                expected: self.revision,
                actual: current.phase_revision,
            });
        }

        if let Some(ref expected_sid) = self.required_session_id {
            match &current.session_id {
                Some(actual_sid) if actual_sid == expected_sid => Ok(()),
                Some(actual_sid) => Err(StaleGenerationError::SessionMismatch {
                    expected: expected_sid.clone(),
                    actual: Some(actual_sid.clone()),
                }),
                None => Err(StaleGenerationError::SessionMismatch {
                    expected: expected_sid.clone(),
                    actual: None,
                }),
            }
        } else {
            Ok(())
        }
    }
}

/// 阶段变迁结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseTransition {
    NoChange,
    PhaseChanged {
        previous: GamePhase,
        current: GamePhase,
        match_epoch: u64,
        phase_revision: u64,
    },
    NewMatchStarted {
        previous: GamePhase,
        current: GamePhase,
        match_epoch: u64,
        phase_revision: u64,
    },
    MatchTerminated {
        previous: GamePhase,
        current: GamePhase,
        match_epoch: u64,
        phase_revision: u64,
    },
}

/// 全局对局生命周期管理器
pub struct LifecycleManager {
    match_epoch: AtomicU64,
    phase_revision: AtomicU64,
    state: RwLock<(GamePhase, Option<String>)>,
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self {
            match_epoch: AtomicU64::new(1),
            phase_revision: AtomicU64::new(0),
            state: RwLock::new((GamePhase::None, None)),
        }
    }

    pub async fn snapshot(&self) -> LifecycleState {
        let (phase, session_id) = self.state.read().await.clone();
        LifecycleState {
            match_epoch: self.match_epoch.load(Ordering::SeqCst),
            phase_revision: self.phase_revision.load(Ordering::SeqCst),
            phase,
            session_id,
        }
    }

    pub async fn guard(&self, require_current_session: bool) -> GenerationGuard {
        let snap = self.snapshot().await;
        GenerationGuard {
            epoch: snap.match_epoch,
            revision: snap.phase_revision,
            required_session_id: if require_current_session {
                snap.session_id
            } else {
                None
            },
        }
    }

    pub async fn on_phase_changed(&self, new_phase_str: &str) -> PhaseTransition {
        let new_phase = GamePhase::from(new_phase_str);
        let mut state = self.state.write().await;
        let (ref mut current_phase, ref mut current_session) = *state;

        if *current_phase == new_phase {
            return PhaseTransition::NoChange;
        }

        let old_phase = current_phase.clone();
        *current_phase = new_phase.clone();

        let old_was_active = old_phase.is_match_active();
        let new_is_active = new_phase.is_match_active();

        let rev = self.phase_revision.fetch_add(1, Ordering::SeqCst) + 1;

        if !old_was_active && new_is_active {
            let ep = self.match_epoch.fetch_add(1, Ordering::SeqCst) + 1;
            PhaseTransition::NewMatchStarted {
                previous: old_phase,
                current: new_phase,
                match_epoch: ep,
                phase_revision: rev,
            }
        } else if old_was_active && !new_is_active {
            *current_session = None;
            let ep = self.match_epoch.load(Ordering::SeqCst);
            PhaseTransition::MatchTerminated {
                previous: old_phase,
                current: new_phase,
                match_epoch: ep,
                phase_revision: rev,
            }
        } else {
            if matches!(new_phase, GamePhase::GameStart | GamePhase::InProgress) {
                *current_session = None;
            }
            let ep = self.match_epoch.load(Ordering::SeqCst);
            PhaseTransition::PhaseChanged {
                previous: old_phase,
                current: new_phase,
                match_epoch: ep,
                phase_revision: rev,
            }
        }
    }

    pub async fn on_session_id_changed(&self, new_session_id: Option<String>) -> bool {
        let mut state = self.state.write().await;
        if state.1 != new_session_id {
            state.1 = new_session_id;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lifecycle_phase_transitions() {
        let lm = LifecycleManager::new();

        // 初始无对局
        let s0 = lm.snapshot().await;
        assert_eq!(s0.phase, GamePhase::None);
        assert_eq!(s0.match_epoch, 1);

        // Lobby -> ReadyCheck (对局开启)
        let t1 = lm.on_phase_changed("ReadyCheck").await;
        match t1 {
            PhaseTransition::NewMatchStarted { match_epoch, phase_revision, .. } => {
                assert_eq!(match_epoch, 2);
                assert_eq!(phase_revision, 1);
            }
            _ => panic!("Expected NewMatchStarted"),
        }

        // ReadyCheck -> ChampSelect (同一对局内部变迁)
        let t2 = lm.on_phase_changed("ChampSelect").await;
        match t2 {
            PhaseTransition::PhaseChanged { match_epoch, phase_revision, .. } => {
                assert_eq!(match_epoch, 2);
                assert_eq!(phase_revision, 2);
            }
            _ => panic!("Expected PhaseChanged"),
        }

        // 产生代际守卫
        let guard = lm.guard(false).await;
        let snap = lm.snapshot().await;
        assert!(guard.ensure_valid(&snap).is_ok());

        // ChampSelect -> InProgress (版本变更，守卫应失效)
        lm.on_phase_changed("InProgress").await;
        let snap2 = lm.snapshot().await;
        assert!(guard.ensure_valid(&snap2).is_err());
    }
}
