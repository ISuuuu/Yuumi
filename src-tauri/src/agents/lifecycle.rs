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
    EpochMismatch {
        expected: u64,
        actual: u64,
    },
    RevisionMismatch {
        expected: u64,
        actual: u64,
    },
    SessionMismatch {
        expected: String,
        actual: Option<String>,
    },
    SessionMissing,
}

impl std::fmt::Display for StaleGenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StaleGenerationError::EpochMismatch { expected, actual } => {
                write!(
                    f,
                    "对局代际不匹配: 期望 epoch={}, 实际={}",
                    expected, actual
                )
            }
            StaleGenerationError::RevisionMismatch { expected, actual } => {
                write!(
                    f,
                    "阶段版本不匹配: 期望 revision={}, 实际={}",
                    expected, actual
                )
            }
            StaleGenerationError::SessionMismatch { expected, actual } => {
                write!(
                    f,
                    "选人会话不匹配: 期望 session={}, 实际={:?}",
                    expected, actual
                )
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
    /// 对局代际：标识一局完整对局生命周期
    pub match_epoch: u64,
    /// 阶段版本号：每次有效进入新阶段递增
    pub phase_revision: u64,
    /// 当前游戏阶段
    pub phase: GamePhase,
    /// 选人会话标识（若处于选人阶段）
    pub session_id: Option<String>,
}

/// 任务代际守卫：在产生外部副作用前验证有效性
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationGuard {
    pub epoch: u64,
    pub revision: u64,
    /// 如果为 Some，则必须严格匹配 session_id
    pub required_session_id: Option<String>,
}

impl GenerationGuard {
    /// 严格断言当前状态是否仍旧匹配守卫约束
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
    /// 阶段无实质变化（重复推送，已去重过滤）
    NoChange,
    /// 同一对局内的正常阶段推演 (例如 ChampSelect -> GameStart)
    PhaseChanged {
        previous: GamePhase,
        current: GamePhase,
        match_epoch: u64,
        phase_revision: u64,
    },
    /// 开启了全新对局 (例如 Lobby -> ReadyCheck / ChampSelect)
    NewMatchStarted {
        previous: GamePhase,
        current: GamePhase,
        match_epoch: u64,
        phase_revision: u64,
    },
    /// 对局彻底结束或重置退出 (例如 EndOfGame -> Lobby / None，或秒退重回 Lobby)
    MatchTerminated {
        previous: GamePhase,
        current: GamePhase,
        finished_epoch: u64,
        phase_revision: u64,
    },
}

impl PhaseTransition {
    /// 取本次变迁的前后阶段；`NoChange`（重复推送被去重过滤）时返回 None。
    /// 供调用方在不穷举全部变体的前提下判断「是否跨入了某个阶段」。
    pub fn phases(&self) -> Option<(&GamePhase, &GamePhase)> {
        match self {
            PhaseTransition::NoChange => None,
            PhaseTransition::PhaseChanged {
                previous, current, ..
            }
            | PhaseTransition::NewMatchStarted {
                previous, current, ..
            }
            | PhaseTransition::MatchTerminated {
                previous, current, ..
            } => Some((previous, current)),
        }
    }
}

/// 统一生命周期管理器
#[derive(Debug)]
pub struct LifecycleManager {
    match_epoch_counter: AtomicU64,
    phase_revision_counter: AtomicU64,
    state: RwLock<LifecycleState>,
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self {
            match_epoch_counter: AtomicU64::new(1),
            phase_revision_counter: AtomicU64::new(1),
            state: RwLock::new(LifecycleState {
                match_epoch: 1,
                phase_revision: 1,
                phase: GamePhase::None,
                session_id: None,
            }),
        }
    }

    /// 获取当前状态快照
    pub async fn snapshot(&self) -> LifecycleState {
        self.state.read().await.clone()
    }

    /// 签发当前代际守卫
    pub async fn capture_guard(&self, require_session: bool) -> GenerationGuard {
        let state = self.state.read().await;
        GenerationGuard {
            epoch: state.match_epoch,
            revision: state.phase_revision,
            required_session_id: if require_session {
                state.session_id.clone()
            } else {
                None
            },
        }
    }

    /// 更新选人会话 ID（当收到 `/lol-champ-select/v1/session` 时更新）
    /// 若会话 ID 发生实质变化，递增 phase_revision 以作废旧会话关联的任务
    pub async fn update_session_id(&self, new_session_id: Option<String>) -> bool {
        let mut state = self.state.write().await;
        if state.session_id != new_session_id {
            state.session_id = new_session_id;
            let new_rev = self.phase_revision_counter.fetch_add(1, Ordering::SeqCst) + 1;
            state.phase_revision = new_rev;
            true
        } else {
            false
        }
    }

    /// 规范化处理阶段变更，执行去重并严格推进 Epoch 与 Revision
    pub async fn update_phase(&self, new_phase_str: &str) -> PhaseTransition {
        let new_phase = GamePhase::from(new_phase_str);
        let mut state = self.state.write().await;

        if state.phase == new_phase {
            return PhaseTransition::NoChange;
        }

        let old_phase = state.phase.clone();

        // 1. 同一对局内的瞬时重连连接态往返 (InProgress/GameStart <-> Reconnect)：
        // 游戏客户端网络抖动或断线重连是同一局游戏内的瞬时连接态，既不产生新对局，
        // 也不推进 revision，使在册任务和代际守卫完全透明存续。
        if (old_phase.is_in_game() && new_phase == GamePhase::Reconnect)
            || (old_phase == GamePhase::Reconnect && new_phase.is_in_game())
        {
            state.phase = new_phase.clone();
            return PhaseTransition::PhaseChanged {
                previous: old_phase,
                current: new_phase,
                match_epoch: state.match_epoch,
                phase_revision: state.phase_revision,
            };
        }

        // 2. 观战中性态：观战不属于玩家活跃对局，进出观战不影响对局代际
        if old_phase == GamePhase::WatchInProgress || new_phase == GamePhase::WatchInProgress {
            state.phase = new_phase.clone();
            return PhaseTransition::PhaseChanged {
                previous: old_phase,
                current: new_phase,
                match_epoch: state.match_epoch,
                phase_revision: state.phase_revision,
            };
        }

        let new_revision = self.phase_revision_counter.fetch_add(1, Ordering::SeqCst) + 1;
        state.phase_revision = new_revision;

        // 判定生命周期边界跃迁（Reconnect 阶段在对局未终结前同样视作活跃对局的一部分）
        let old_is_match = old_phase.is_match_active() || old_phase == GamePhase::Reconnect;
        let new_is_match = new_phase.is_match_active() || new_phase == GamePhase::Reconnect;

        let transition = if !old_is_match && new_is_match {
            // 从非对局阶段进入对局活跃阶段（新对局诞生）
            let new_epoch = self.match_epoch_counter.fetch_add(1, Ordering::SeqCst) + 1;
            state.match_epoch = new_epoch;
            state.phase = new_phase.clone();
            if new_phase == GamePhase::ChampSelect && state.session_id.is_none() {
                state.session_id = Some(format!("cs-{}-{}", new_epoch, new_revision));
            }
            PhaseTransition::NewMatchStarted {
                previous: old_phase,
                current: new_phase,
                match_epoch: new_epoch,
                phase_revision: new_revision,
            }
        } else if old_is_match && !new_is_match {
            // 对局结束或秒退，脱离活跃对局状态（包括掉线重连期间游戏结束退出到 Lobby/None）
            let finished_epoch = state.match_epoch;
            state.phase = new_phase.clone();
            state.session_id = None; // 退出选人/对局会话
            PhaseTransition::MatchTerminated {
                previous: old_phase,
                current: new_phase,
                finished_epoch,
                phase_revision: new_revision,
            }
        } else {
            // 同一活跃对局或同处于空闲期内的阶段演进
            state.phase = new_phase.clone();
            if new_phase != GamePhase::ChampSelect {
                state.session_id = None;
            } else if state.session_id.is_none() {
                state.session_id = Some(format!("cs-{}-{}", state.match_epoch, new_revision));
            }
            PhaseTransition::PhaseChanged {
                previous: old_phase,
                current: new_phase,
                match_epoch: state.match_epoch,
                phase_revision: new_revision,
            }
        };

        transition
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_phase_transition_and_deduplication() {
        let manager = LifecycleManager::new();

        // 初始状态
        let snap0 = manager.snapshot().await;
        assert_eq!(snap0.phase, GamePhase::None);
        assert_eq!(snap0.match_epoch, 1);
        assert_eq!(snap0.phase_revision, 1);

        // 重复推送 None，返回 NoChange
        let trans1 = manager.update_phase("None").await;
        assert_eq!(trans1, PhaseTransition::NoChange);

        // 进入 Lobby，同处于非活跃期
        let trans2 = manager.update_phase("Lobby").await;
        match trans2 {
            PhaseTransition::PhaseChanged {
                previous,
                current,
                match_epoch,
                phase_revision,
            } => {
                assert_eq!(previous, GamePhase::None);
                assert_eq!(current, GamePhase::Lobby);
                assert_eq!(match_epoch, 1);
                assert_eq!(phase_revision, 2);
            }
            _ => panic!("Expected PhaseChanged"),
        }

        // 重复推送 Lobby
        assert_eq!(
            manager.update_phase("Lobby").await,
            PhaseTransition::NoChange
        );

        // 进入 ReadyCheck，跨入活跃对局期，触发 NewMatchStarted 并递增 match_epoch
        let trans3 = manager.update_phase("ReadyCheck").await;
        match trans3 {
            PhaseTransition::NewMatchStarted {
                match_epoch,
                phase_revision,
                ..
            } => {
                assert_eq!(match_epoch, 2);
                assert_eq!(phase_revision, 3);
            }
            _ => panic!("Expected NewMatchStarted"),
        }

        // 同局推进到 ChampSelect
        let trans4 = manager.update_phase("ChampSelect").await;
        match trans4 {
            PhaseTransition::PhaseChanged {
                match_epoch,
                phase_revision,
                ..
            } => {
                assert_eq!(match_epoch, 2); // 保持同一局 epoch
                assert_eq!(phase_revision, 4); // 推进 revision
            }
            _ => panic!("Expected PhaseChanged"),
        }

        // 秒退回 Lobby，对局终结
        let trans5 = manager.update_phase("Lobby").await;
        match trans5 {
            PhaseTransition::MatchTerminated {
                finished_epoch,
                phase_revision,
                ..
            } => {
                assert_eq!(finished_epoch, 2);
                assert_eq!(phase_revision, 5);
            }
            _ => panic!("Expected MatchTerminated"),
        }
    }

    #[tokio::test]
    async fn test_neutral_phases_do_not_bump_match_epoch() {
        let manager = LifecycleManager::new();

        // 进入对局
        manager.update_phase("InProgress").await;
        let before = manager.snapshot().await;
        assert_eq!(before.match_epoch, 2);
        assert_eq!(before.phase, GamePhase::InProgress);

        // 游戏中断线重连：InProgress -> Reconnect -> InProgress 必须是同一局
        let t1 = manager.update_phase("Reconnect").await;
        assert!(
            matches!(t1, PhaseTransition::PhaseChanged { .. }),
            "Reconnect 不应被判定为对局终结"
        );
        assert_eq!(manager.snapshot().await.match_epoch, 2);

        let t2 = manager.update_phase("InProgress").await;
        assert!(
            matches!(t2, PhaseTransition::PhaseChanged { .. }),
            "从 Reconnect 回到 InProgress 不应被判定为新对局诞生"
        );
        let after = manager.snapshot().await;
        assert_eq!(after.match_epoch, 2, "重连往返不得虚增对局代际");
        assert_eq!(after.phase, GamePhase::InProgress);

        // 观战同样是中性态，None -> WatchInProgress -> None 不产生新对局
        manager.update_phase("None").await;
        manager.update_phase("WatchInProgress").await;
        assert_eq!(manager.snapshot().await.match_epoch, 2);
        manager.update_phase("None").await;

        // 真正的新对局仍然正常递增
        let t3 = manager.update_phase("ReadyCheck").await;
        assert!(
            matches!(t3, PhaseTransition::NewMatchStarted { .. }),
            "中性态修复不能影响真实的新对局跃迁"
        );
        assert_eq!(manager.snapshot().await.match_epoch, 3);
    }

    #[tokio::test]
    async fn test_generation_guard_survives_reconnect_blip() {
        let manager = LifecycleManager::new();
        manager.update_phase("InProgress").await;

        // 重连闪烁期间签发的守卫，重连结束后必须仍然有效
        let guard = manager.capture_guard(false).await;
        manager.update_phase("Reconnect").await;
        manager.update_phase("InProgress").await;
        assert!(guard.ensure_valid(&manager.snapshot().await).is_ok());

        // 但真正开启新对局后必须失效
        manager.update_phase("Lobby").await;
        manager.update_phase("ChampSelect").await;
        assert!(guard.ensure_valid(&manager.snapshot().await).is_err());
    }

    #[tokio::test]
    async fn test_reconnect_to_lobby_terminates_match_and_invalidates_guard() {
        let manager = LifecycleManager::new();
        manager.update_phase("InProgress").await;
        let guard = manager.capture_guard(false).await;

        // 掉线进入 Reconnect，守卫仍然有效
        manager.update_phase("Reconnect").await;
        assert!(guard.ensure_valid(&manager.snapshot().await).is_ok());

        // 掉线期间对局结束回到 Lobby，必须触发 MatchTerminated 且使守卫失效
        let t = manager.update_phase("Lobby").await;
        assert!(
            matches!(t, PhaseTransition::MatchTerminated { .. }),
            "从 Reconnect 退出到 Lobby 必须判定为对局终结"
        );
        assert!(
            guard.ensure_valid(&manager.snapshot().await).is_err(),
            "对局终结后 revision 推进，守卫必须失效"
        );
    }

    #[tokio::test]
    async fn test_generation_guard_strict_session_validation() {
        let manager = LifecycleManager::new();
        manager.update_phase("ChampSelect").await;
        manager
            .update_session_id(Some("session-1001".to_string()))
            .await;

        let guard = manager.capture_guard(true).await;
        assert_eq!(guard.required_session_id.as_deref(), Some("session-1001"));

        // 当前状态完全一致，校验通过
        let current_snap = manager.snapshot().await;
        assert!(guard.ensure_valid(&current_snap).is_ok());

        // 模拟外部事件：当前会话变成了 None（退出或刷新），即使 epoch 和 revision 相同也必须拦截！
        let mut tampered_snap = current_snap.clone();
        tampered_snap.session_id = None;
        assert_eq!(
            guard.ensure_valid(&tampered_snap),
            Err(StaleGenerationError::SessionMismatch {
                expected: "session-1001".to_string(),
                actual: None
            })
        );

        // 模拟外部事件：换了新 session
        tampered_snap.session_id = Some("session-2002".to_string());
        assert_eq!(
            guard.ensure_valid(&tampered_snap),
            Err(StaleGenerationError::SessionMismatch {
                expected: "session-1001".to_string(),
                actual: Some("session-2002".to_string())
            })
        );
    }
}
