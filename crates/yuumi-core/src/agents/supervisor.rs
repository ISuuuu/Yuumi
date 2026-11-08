//! 自动化 Agent 任务调度器与独占许可池（继承 commit 6c16a7f 独占许可防护机制）

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

pub type TaskId = u64;

const REPLACE_GRACE_TIMEOUT: Duration = Duration::from_millis(1000);

/// 任务类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskKind {
    AutoPickLock,
    AutoBanLock,
    AutoSwap,
    AutoTrade,
    AutoAccept,
    AutoPlayAgain,
    AutoReconnect,
    AutoHonor,
    AutoSpell,
    AramBenchSwap,
    Other(String),
}

/// 重复触发策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateStrategy {
    /// 替换旧任务：取消正在执行的同类旧任务，启动新任务
    ReplaceOld,
    /// 拒绝新任务：若已有同类任务正在执行，放弃本次触发
    RejectNew,
    /// 允许并发执行
    AllowConcurrent,
}

impl TaskKind {
    pub fn default_strategy(&self) -> DuplicateStrategy {
        match self {
            TaskKind::AutoPickLock
            | TaskKind::AutoSpell
            | TaskKind::AutoSwap
            | TaskKind::AutoTrade => DuplicateStrategy::ReplaceOld,
            TaskKind::AutoBanLock
            | TaskKind::AutoAccept
            | TaskKind::AutoPlayAgain
            | TaskKind::AutoReconnect => DuplicateStrategy::RejectNew,
            TaskKind::AutoHonor | TaskKind::AramBenchSwap | TaskKind::Other(_) => {
                DuplicateStrategy::AllowConcurrent
            }
        }
    }
}

/// 任务启动错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnError {
    DuplicateRejected(TaskKind),
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpawnError::DuplicateRejected(kind) => {
                write!(f, "任务被拒绝: 同类型任务 {:?} 正在运行", kind)
            }
        }
    }
}

impl std::error::Error for SpawnError {}

struct RunningTaskInfo {
    id: TaskId,
    cancel_token: CancellationToken,
    done_rx: Option<oneshot::Receiver<()>>,
}

/// 任务上下文：传递给任务闭包
#[derive(Clone)]
pub struct TaskContext {
    pub id: TaskId,
    pub kind: TaskKind,
    pub cancel_token: CancellationToken,
}

impl TaskContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

/// 全局任务调度器
#[derive(Clone)]
pub struct AgentSupervisor {
    next_id: Arc<AtomicU64>,
    running: Arc<Mutex<HashMap<TaskKind, RunningTaskInfo>>>,
    agent_exclusive_semaphore: Arc<Semaphore>,
}

impl Default for AgentSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSupervisor {
    pub fn new() -> Self {
        Self {
            next_id: Arc::new(AtomicU64::new(1)),
            running: Arc::new(Mutex::new(HashMap::new())),
            // 独占信号量：容量为 1，确保关键写操作（选人/接受/禁人）全局互斥
            agent_exclusive_semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    /// 获取 Agent 独占执行许可（commit 6c16a7f 防护：避免并发冲突）
    pub async fn acquire_exclusive_permit(&self) -> OwnedSemaphorePermit {
        self.agent_exclusive_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("独占许可池关闭异常")
    }

    /// 尝试以非阻塞方式获取 Agent 独占执行许可
    pub fn try_acquire_exclusive_permit(&self) -> Option<OwnedSemaphorePermit> {
        self.agent_exclusive_semaphore
            .clone()
            .try_acquire_owned()
            .ok()
    }

    /// 调度执行异步任务
    pub async fn spawn<F, Fut>(
        &self,
        kind: TaskKind,
        strategy: DuplicateStrategy,
        task_fn: F,
    ) -> Result<TaskId, SpawnError>
    where
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let cancel_token = CancellationToken::new();
        let (done_tx, done_rx) = oneshot::channel::<()>();

        match strategy {
            DuplicateStrategy::AllowConcurrent => {
                let ctx = TaskContext {
                    id,
                    kind,
                    cancel_token,
                };
                tokio::spawn(async move {
                    task_fn(ctx).await;
                    let _ = done_tx.send(());
                });
                Ok(id)
            }
            DuplicateStrategy::RejectNew => {
                {
                    let mut map = self.running.lock().await;
                    if map.contains_key(&kind) {
                        return Err(SpawnError::DuplicateRejected(kind));
                    }
                    map.insert(
                        kind.clone(),
                        RunningTaskInfo {
                            id,
                            cancel_token: cancel_token.clone(),
                            done_rx: Some(done_rx),
                        },
                    );
                }
                self.spawn_tracked(id, kind, cancel_token, task_fn, done_tx);
                Ok(id)
            }
            DuplicateStrategy::ReplaceOld => {
                let old_to_wait = {
                    let mut map = self.running.lock().await;
                    if let Some(mut old) = map.remove(&kind) {
                        old.cancel_token.cancel();
                        old.done_rx.take()
                    } else {
                        None
                    }
                };

                if let Some(rx) = old_to_wait {
                    let _ = tokio::time::timeout(REPLACE_GRACE_TIMEOUT, rx).await;
                }

                {
                    let mut map = self.running.lock().await;
                    map.insert(
                        kind.clone(),
                        RunningTaskInfo {
                            id,
                            cancel_token: cancel_token.clone(),
                            done_rx: Some(done_rx),
                        },
                    );
                }
                self.spawn_tracked(id, kind, cancel_token, task_fn, done_tx);
                Ok(id)
            }
        }
    }

    /// 启动已被登记的异步任务，并在其结束后回收登记表条目。
    ///
    /// 回收是幂等性的关键：若条目永久滞留，后续同类 `RejectNew` 任务会被永久拒绝，
    /// 也会让 `running` 表随时间无限增长（通道与状态泄漏）。
    fn spawn_tracked<F, Fut>(
        &self,
        id: TaskId,
        kind: TaskKind,
        cancel_token: CancellationToken,
        task_fn: F,
        done_tx: oneshot::Sender<()>,
    ) where
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let running = self.running.clone();
        let ctx = TaskContext {
            id,
            kind: kind.clone(),
            cancel_token,
        };
        tokio::spawn(async move {
            task_fn(ctx).await;
            {
                let mut map = running.lock().await;
                // 仅当登记条目仍是本任务时才回收，避免误删「替换后」的新任务
                if map.get(&kind).map(|info| info.id) == Some(id) {
                    map.remove(&kind);
                }
            }
            let _ = done_tx.send(());
        });
    }

    /// 当前登记在案的任务数量（用于泄漏自检与测试断言）
    pub async fn running_task_count(&self) -> usize {
        self.running.lock().await.len()
    }

    /// 取消指定类型的任务
    pub async fn cancel(&self, kind: &TaskKind) -> bool {
        let mut map = self.running.lock().await;
        if let Some(info) = map.remove(kind) {
            info.cancel_token.cancel();
            true
        } else {
            false
        }
    }

    /// 取消所有正在运行的自动化任务（例如游戏结束或重置时）
    pub async fn cancel_all(&self) {
        let mut map = self.running.lock().await;
        for (_, info) in map.drain() {
            info.cancel_token.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_exclusive_permit() {
        let supervisor = AgentSupervisor::new();
        let permit1 = supervisor.acquire_exclusive_permit().await;
        assert!(supervisor.try_acquire_exclusive_permit().is_none());
        drop(permit1);
        assert!(supervisor.try_acquire_exclusive_permit().is_some());
    }

    #[tokio::test]
    async fn test_supervisor_reject_duplicate() {
        let supervisor = AgentSupervisor::new();
        let r1 = supervisor
            .spawn(
                TaskKind::AutoAccept,
                DuplicateStrategy::RejectNew,
                |_| async {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                },
            )
            .await;
        assert!(r1.is_ok());

        let r2 = supervisor
            .spawn(
                TaskKind::AutoAccept,
                DuplicateStrategy::RejectNew,
                |_| async {},
            )
            .await;
        assert!(matches!(r2, Err(SpawnError::DuplicateRejected(_))));
    }

    #[tokio::test]
    async fn test_reject_new_entry_reclaimed_after_completion() {
        let supervisor = AgentSupervisor::new();
        supervisor
            .spawn(
                TaskKind::AutoAccept,
                DuplicateStrategy::RejectNew,
                |_| async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                },
            )
            .await
            .expect("首次触发应当成功");
        assert_eq!(supervisor.running_task_count().await, 1);

        // 轮询等待任务结束并完成登记回收，避免依赖固定 sleep 时长
        let mut reclaimed = false;
        for _ in 0..100 {
            if supervisor.running_task_count().await == 0 {
                reclaimed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            reclaimed,
            "任务结束后登记条目必须被回收，否则后续同类任务会被永久拒绝"
        );

        // 回收后允许再次触发同类任务（幂等重试）
        assert!(supervisor
            .spawn(
                TaskKind::AutoAccept,
                DuplicateStrategy::RejectNew,
                |_| async {}
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_replace_old_does_not_drop_new_entry() {
        let supervisor = AgentSupervisor::new();
        supervisor
            .spawn(
                TaskKind::AutoPickLock,
                DuplicateStrategy::ReplaceOld,
                |_| async {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                },
            )
            .await
            .expect("首个任务应当成功");

        // 替换为长任务，旧任务被取消后会尝试回收，但不得误删新任务的登记
        supervisor
            .spawn(
                TaskKind::AutoPickLock,
                DuplicateStrategy::ReplaceOld,
                |_| async {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                },
            )
            .await
            .expect("替换应当成功");

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(supervisor.running_task_count().await, 1);

        supervisor.cancel_all().await;
    }
}
