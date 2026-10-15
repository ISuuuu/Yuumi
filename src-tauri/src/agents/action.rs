use super::auto_bp::ChampSelectSession;
use super::lifecycle::{GenerationGuard, LifecycleState, StaleGenerationError};

/// 写操作目标验证错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionVerificationError {
    /// 生命周期或代际失效
    LifecycleStale(StaleGenerationError),
    /// 未能在最新会话中找到对应的 Action
    ActionNotFound(i32),
    /// Action 类型不匹配（例如期望 pick 变成了 ban）
    ActionTypeMismatch { expected: String, actual: String },
    /// Action 对应的席位不匹配
    ActorMismatch { expected: i32, actual: i32 },
    /// Action 已经处于完成状态，无需/不可重复提交
    ActionAlreadyCompleted(i32),
}

impl std::fmt::Display for ActionVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionVerificationError::LifecycleStale(e) => write!(f, "生命周期失效: {}", e),
            ActionVerificationError::ActionNotFound(id) => write!(f, "未找到目标 Action: {}", id),
            ActionVerificationError::ActionTypeMismatch { expected, actual } => {
                write!(f, "Action 类型不匹配: 期望={}, 实际={}", expected, actual)
            }
            ActionVerificationError::ActorMismatch { expected, actual } => {
                write!(f, "Action 席位不匹配: 期望={}, 实际={}", expected, actual)
            }
            ActionVerificationError::ActionAlreadyCompleted(id) => {
                write!(f, "Action {} 已完成，拒绝重复写入", id)
            }
        }
    }
}

impl std::error::Error for ActionVerificationError {}

impl From<StaleGenerationError> for ActionVerificationError {
    fn from(e: StaleGenerationError) -> Self {
        ActionVerificationError::LifecycleStale(e)
    }
}

/// 受保护的 BP 操作租约 (ActionLease)
/// 绑定了生命周期 Guard 与具体操作槽位，在写操作前做终极双重断言
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionLease {
    pub guard: GenerationGuard,
    pub action_id: i32,
    pub actor_cell_id: i32,
    pub action_type: String,
    pub expected_champion_id: i32,
}

impl ActionLease {
    pub fn new(
        guard: GenerationGuard,
        action_id: i32,
        actor_cell_id: i32,
        action_type: String,
        expected_champion_id: i32,
    ) -> Self {
        Self {
            guard,
            action_id,
            actor_cell_id,
            action_type,
            expected_champion_id,
        }
    }

    /// 在真正向 LCU 发送写操作之前，结合最新的生命周期状态与最新 session 结构进行终极双重断言
    pub fn verify_target(
        &self,
        current_lifecycle: &LifecycleState,
        fresh_session: &ChampSelectSession,
    ) -> Result<(), ActionVerificationError> {
        // 1. 先验证生命周期代际与 session 是否严格有效
        self.guard.ensure_valid(current_lifecycle)?;

        // 2. 验证席位一致性
        if fresh_session.local_player_cell_id != self.actor_cell_id {
            return Err(ActionVerificationError::ActorMismatch {
                expected: self.actor_cell_id,
                actual: fresh_session.local_player_cell_id,
            });
        }

        // 3. 寻找 fresh_session 中的目标 action 并严格比对属性
        let target_action = fresh_session
            .actions
            .iter()
            .flatten()
            .find(|a| a.id == self.action_id);

        let action = match target_action {
            Some(a) => a,
            None => return Err(ActionVerificationError::ActionNotFound(self.action_id)),
        };

        if action.actor_cell_id != self.actor_cell_id {
            return Err(ActionVerificationError::ActorMismatch {
                expected: self.actor_cell_id,
                actual: action.actor_cell_id,
            });
        }

        if action.action_type != self.action_type {
            return Err(ActionVerificationError::ActionTypeMismatch {
                expected: self.action_type.clone(),
                actual: action.action_type.clone(),
            });
        }

        if action.completed {
            return Err(ActionVerificationError::ActionAlreadyCompleted(
                self.action_id,
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::auto_bp::{ChampSelectAction, ChampSelectBans};
    use crate::agents::lifecycle::GamePhase;

    fn mock_session(action_id: i32, cell_id: i32, completed: bool) -> ChampSelectSession {
        ChampSelectSession {
            actions: vec![vec![ChampSelectAction {
                actor_cell_id: cell_id,
                champion_id: 103, // Ahri
                completed,
                id: action_id,
                is_in_progress: true,
                action_type: "pick".to_string(),
            }]],
            local_player_cell_id: cell_id,
            my_team: vec![],
            bans: ChampSelectBans::default(),
            pick_order_swaps: vec![],
            trades: vec![],
            timer: None,
            bench_enabled: false,
            queue_id: Some(420),
        }
    }

    #[test]
    fn test_action_lease_verification() {
        let guard = GenerationGuard {
            epoch: 2,
            revision: 4,
            required_session_id: Some("session-abc".to_string()),
        };

        let lease = ActionLease::new(guard, 10, 2, "pick".to_string(), 103);

        let lifecycle = LifecycleState {
            match_epoch: 2,
            phase_revision: 4,
            phase: GamePhase::ChampSelect,
            session_id: Some("session-abc".to_string()),
        };

        let session = mock_session(10, 2, false);

        // 正常场景：全部匹配，校验通过
        assert!(lease.verify_target(&lifecycle, &session).is_ok());

        // 场景 A：Action 已经被他人或自己完成 (completed=true)，拒绝重复写入
        let completed_session = mock_session(10, 2, true);
        assert_eq!(
            lease.verify_target(&lifecycle, &completed_session),
            Err(ActionVerificationError::ActionAlreadyCompleted(10))
        );

        // 场景 B：席位变动（例如重开后相同 action_id 但属于他人）
        let foreign_session = mock_session(10, 3, false);
        assert!(matches!(
            lease.verify_target(&lifecycle, &foreign_session),
            Err(ActionVerificationError::ActorMismatch { .. })
        ));

        // 场景 C：对局生命周期失效（例如秒退）
        let mut stale_lifecycle = lifecycle.clone();
        stale_lifecycle.phase_revision = 5;
        assert!(matches!(
            lease.verify_target(&stale_lifecycle, &session),
            Err(ActionVerificationError::LifecycleStale(_))
        ));
    }
}
