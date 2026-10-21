use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tokio_util::sync::CancellationToken;

pub type TaskId = u64;

/// ReplaceOld 策略下等待旧任务真正退出的上限。
/// 协作式取消需要旧任务自己跑到下一个取消检查点；若它正阻塞在一次 LCU 写请求上，
/// 无限等待会拖死选人倒计时，因此设上限并在超时后放行新任务。
const REPLACE_GRACE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1000);

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
    /// 默认去重策略
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
    /// 因已有同类任务正在运行，被去重策略拒绝
    DuplicateRejected(TaskKind),
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpawnError::DuplicateRejected(kind) => {
                write!(f, "任务启动被拒绝: 已有同类任务正在运行 ({:?})", kind)
            }
        }
    }
}

impl std::error::Error for SpawnError {}

/// 在册运行中的任务信息快照
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    pub id: TaskId,
    pub kind: TaskKind,
    pub description: String,
}

/// 在册运行中的任务句柄
struct TaskEntry {
    kind: TaskKind,
    description: String,
    token: CancellationToken,
    /// 任务退出信号：正常结束时显式通知，panic 时 sender 随栈展开释放同样会唤醒等待方。
    /// 供 ReplaceOld 策略等待旧任务真正让出执行权后再启动新任务。
    done: oneshot::Receiver<()>,
}

/// 异步任务监管中心
#[derive(Clone)]
pub struct TaskSupervisor {
    next_id: Arc<AtomicU64>,
    tasks: Arc<Mutex<HashMap<TaskId, TaskEntry>>>,
}

impl Default for TaskSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskSupervisor {
    pub fn new() -> Self {
        Self {
            next_id: Arc::new(AtomicU64::new(1)),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册并启动一个受监管的异步任务
    pub async fn spawn<F, Fut>(
        &self,
        kind: TaskKind,
        desc: impl Into<String>,
        task_fn: F,
    ) -> Result<TaskId, SpawnError>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let strategy = kind.default_strategy();
        let desc_str = desc.into();

        // ── 阶段一：按去重策略处理同类在册任务，并摘下需要等待其真正退出的旧任务
        let mut replaced: Vec<(TaskId, oneshot::Receiver<()>)> = Vec::new();
        {
            let mut tasks_lock = self.tasks.lock().await;

            match strategy {
                DuplicateStrategy::RejectNew => {
                    let already_running = tasks_lock.values().any(|t| t.kind == kind);
                    if already_running {
                        return Err(SpawnError::DuplicateRejected(kind));
                    }
                }
                DuplicateStrategy::ReplaceOld => {
                    // 找出同类任务并通知取消
                    let old_task_ids: Vec<TaskId> = tasks_lock
                        .iter()
                        .filter(|(_, t)| t.kind == kind)
                        .map(|(id, _)| *id)
                        .collect();

                    for old_id in old_task_ids {
                        if let Some(entry) = tasks_lock.remove(&old_id) {
                            entry.token.cancel();
                            replaced.push((old_id, entry.done));
                        }
                    }
                }
                DuplicateStrategy::AllowConcurrent => {}
            }
        }

        // ── 阶段二：等待旧任务真正退出。
        // 取消是协作式的，仅 remove + cancel 就立刻启动新任务会让两次 LCU 写并发命中同一 action
        // （自动选人/技能/交换/交易皆为此类），因此在锁外等它收尾，超时兜底放行。
        for (old_id, done) in replaced {
            match tokio::time::timeout(REPLACE_GRACE_TIMEOUT, done).await {
                Ok(Ok(())) => log::debug!("[supervisor] 旧任务 #{} 已退出，接力启动新任务", old_id),
                // 旧任务 panic 时 sender 提前释放，同样视为已退出
                Ok(Err(_)) => log::debug!("[supervisor] 旧任务 #{} 已终止（信号提前释放）", old_id),
                Err(_) => log::warn!(
                    "[supervisor] 旧任务 #{} 在 {:?} 内未退出，仍继续启动新任务",
                    old_id,
                    REPLACE_GRACE_TIMEOUT
                ),
            }
        }

        // ── 阶段三：入册并启动
        let task_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let token = CancellationToken::new();
        let (done_tx, done_rx) = oneshot::channel();

        {
            let mut tasks_lock = self.tasks.lock().await;

            // 阶段二等待期间可能有同类任务插队入册（并发触发），让位于本次最新请求
            if matches!(strategy, DuplicateStrategy::ReplaceOld) {
                let racing_ids: Vec<TaskId> = tasks_lock
                    .iter()
                    .filter(|(_, t)| t.kind == kind)
                    .map(|(id, _)| *id)
                    .collect();
                for racing_id in racing_ids {
                    if let Some(entry) = tasks_lock.remove(&racing_id) {
                        entry.token.cancel();
                    }
                }
            }

            tasks_lock.insert(
                task_id,
                TaskEntry {
                    kind,
                    description: desc_str,
                    token: token.clone(),
                    done: done_rx,
                },
            );
        }

        // 任务正常结束时自行摘除在册记录；Drop 作为任务 panic/unwind 时的兜底。
        let tasks_ref = Arc::clone(&self.tasks);
        struct TaskCleanupGuard {
            task_id: TaskId,
            tasks: Arc<Mutex<HashMap<TaskId, TaskEntry>>>,
        }
        impl Drop for TaskCleanupGuard {
            fn drop(&mut self) {
                // 快速路径：无竞争时同步直接移除
                if let Ok(mut tasks) = self.tasks.try_lock() {
                    tasks.remove(&self.task_id);
                    return;
                }
                // 兜底路径：存在锁竞争时，利用当前 runtime 异步等待锁并移除；
                // 仅在 runtime 存活时 spawn，规避 shutdown 期 double panic
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let tasks = Arc::clone(&self.tasks);
                    let task_id = self.task_id;
                    handle.spawn(async move {
                        tasks.lock().await.remove(&task_id);
                    });
                }
            }
        }

        tokio::spawn(async move {
            let _guard = TaskCleanupGuard {
                task_id,
                tasks: Arc::clone(&tasks_ref),
            };
            task_fn(token).await;
            tasks_ref.lock().await.remove(&task_id);
            // 在册记录摘除后再通知等待方，确保 ReplaceOld 的继任任务看到的是干净的注册表
            let _ = done_tx.send(());
        });

        Ok(task_id)
    }

    /// 精确取消指定任务
    pub async fn cancel(&self, id: TaskId) -> bool {
        let mut tasks = self.tasks.lock().await;
        if let Some(entry) = tasks.remove(&id) {
            entry.token.cancel();
            true
        } else {
            false
        }
    }

    /// 取消指定类型的所有任务，返回取消的任务数
    pub async fn cancel_kind(&self, kind: &TaskKind) -> usize {
        let mut tasks = self.tasks.lock().await;
        let matched_ids: Vec<TaskId> = tasks
            .iter()
            .filter(|(_, t)| &t.kind == kind)
            .map(|(id, _)| *id)
            .collect();

        let count = matched_ids.len();
        for id in matched_ids {
            if let Some(entry) = tasks.remove(&id) {
                entry.token.cancel();
            }
        }
        count
    }

    /// 取消当前全部任务并清空（如对局结束、秒退重置、LCU 断开时调用）
    pub async fn cancel_all(&self) -> usize {
        let mut tasks = self.tasks.lock().await;
        let count = tasks.len();
        for (_, entry) in tasks.drain() {
            entry.token.cancel();
        }
        count
    }

    /// 获取当前在册任务数量
    pub async fn active_count(&self) -> usize {
        self.tasks.lock().await.len()
    }

    /// 检查指定任务是否在册
    pub async fn is_running(&self, id: TaskId) -> bool {
        self.tasks.lock().await.contains_key(&id)
    }

    /// 检查是否存在指定类型的活跃任务
    pub async fn has_active_kind(&self, kind: &TaskKind) -> bool {
        self.tasks.lock().await.values().any(|t| &t.kind == kind)
    }

    /// 列出所有正在运行中的任务信息
    pub async fn list_tasks(&self) -> Vec<TaskSnapshot> {
        let tasks = self.tasks.lock().await;
        tasks
            .iter()
            .map(|(id, entry)| TaskSnapshot {
                id: *id,
                kind: entry.kind.clone(),
                description: entry.description.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_supervisor_replace_old() {
        let supervisor = TaskSupervisor::new();

        // 启动第一个 AutoPickLock 任务
        let task1 = supervisor
            .spawn(
                TaskKind::AutoPickLock,
                "Pick Yasuo",
                |token: CancellationToken| async move {
                    tokio::select! {
                        _ = sleep(Duration::from_millis(500)) => {}
                        _ = token.cancelled() => {}
                    }
                },
            )
            .await
            .unwrap();

        assert_eq!(supervisor.active_count().await, 1);
        assert!(supervisor.is_running(task1).await);

        // 启动第二个 AutoPickLock 任务，应取消第一个并替换
        let task2 = supervisor
            .spawn(
                TaskKind::AutoPickLock,
                "Pick Yone",
                |token: CancellationToken| async move {
                    tokio::select! {
                        _ = sleep(Duration::from_millis(500)) => {}
                        _ = token.cancelled() => {}
                    }
                },
            )
            .await
            .unwrap();

        assert_ne!(task1, task2);
        // task1 应该已经被移出并取消
        assert!(!supervisor.is_running(task1).await);
        assert!(supervisor.is_running(task2).await);
        assert_eq!(supervisor.active_count().await, 1);
    }

    #[tokio::test]
    async fn test_supervisor_reject_new() {
        let supervisor = TaskSupervisor::new();

        // 启动第一个 AutoAccept
        let task1 = supervisor
            .spawn(
                TaskKind::AutoAccept,
                "Accept Match",
                |token: CancellationToken| async move {
                    tokio::select! {
                        _ = sleep(Duration::from_millis(200)) => {}
                        _ = token.cancelled() => {}
                    }
                },
            )
            .await
            .unwrap();

        // 紧接着尝试启动第二个 AutoAccept，应该被拒绝
        let res2 = supervisor
            .spawn(
                TaskKind::AutoAccept,
                "Accept Match Duplicate",
                |_| async move {},
            )
            .await;

        assert_eq!(
            res2.err(),
            Some(SpawnError::DuplicateRejected(TaskKind::AutoAccept))
        );
        assert_eq!(supervisor.active_count().await, 1);
        assert!(supervisor.is_running(task1).await);
    }

    #[tokio::test]
    async fn test_supervisor_auto_cleanup_on_finish() {
        let supervisor = TaskSupervisor::new();

        let task1 = supervisor
            .spawn(TaskKind::AutoSpell, "Fast Spell", |_| async move {
                // 极快执行完毕
            })
            .await
            .unwrap();

        // 等待轻微异步延迟以让 drop guard 生效
        sleep(Duration::from_millis(50)).await;
        assert!(!supervisor.is_running(task1).await);
        assert_eq!(supervisor.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_supervisor_cancel_all() {
        let supervisor = TaskSupervisor::new();

        let _ = supervisor
            .spawn(TaskKind::AutoPickLock, "Task 1", |token| async move {
                token.cancelled().await
            })
            .await;
        let _ = supervisor
            .spawn(TaskKind::AutoAccept, "Task 2", |token| async move {
                token.cancelled().await
            })
            .await;

        assert_eq!(supervisor.active_count().await, 2);

        let cancelled = supervisor.cancel_all().await;
        assert_eq!(cancelled, 2);
        assert_eq!(supervisor.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_replace_old_waits_previous_task_exit() {
        use std::sync::Mutex as StdMutex;

        let supervisor = TaskSupervisor::new();
        let order: Arc<StdMutex<Vec<&'static str>>> = Arc::new(StdMutex::new(Vec::new()));

        // 旧任务：收到取消后仍需一段时间收尾（模拟已发出的 LCU 写请求仍在途）
        let old_order = Arc::clone(&order);
        supervisor
            .spawn(
                TaskKind::AutoPickLock,
                "old pick",
                move |token: CancellationToken| async move {
                    token.cancelled().await;
                    sleep(Duration::from_millis(40)).await;
                    if let Ok(mut o) = old_order.lock() {
                        o.push("old_exit");
                    }
                },
            )
            .await
            .unwrap();

        // 替换任务必须等旧任务真正退出后才开始执行，杜绝两次写并发命中同一 action
        let new_order = Arc::clone(&order);
        supervisor
            .spawn(TaskKind::AutoPickLock, "new pick", move |_| async move {
                if let Ok(mut o) = new_order.lock() {
                    o.push("new_start");
                }
            })
            .await
            .unwrap();

        sleep(Duration::from_millis(80)).await;
        let recorded = order.lock().expect("order lock").clone();
        assert_eq!(recorded, vec!["old_exit", "new_start"]);
        assert_eq!(supervisor.active_count().await, 0);
    }
}
