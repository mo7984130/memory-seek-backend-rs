use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Local;
use dashmap::DashMap;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace};

/// 定时任务调度规格
#[derive(Debug, Clone, Copy)]
pub enum TaskSchedule {
    /// 每天本地时间 (hour:minute) 触发一次
    Daily { hour: u32, minute: u32 },
}

#[derive(Clone)]
pub struct TaskManager {
    inner: Arc<TaskManagerInner>,
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

struct TaskManagerInner {
    /// 任务名 -> 同名任务列表。同名任务允许共存，互不覆盖。
    tasks: DashMap<String, Vec<JoinHandle<()>>>,
    cancel_token: CancellationToken,
    /// `wait_all` 当前正在等待的任务组名（超时诊断用）
    waiting: Mutex<Option<String>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self::with_token(CancellationToken::new())
    }

    /// 传入根取消令牌构造。
    ///
    /// 应用启动时通常传入 `AppSetup.cancel_token`，使所有任务与应用共享
    /// 同一个取消源，关闭时一次取消即可全部通知。
    pub fn with_token(cancel_token: CancellationToken) -> Self {
        Self {
            inner: Arc::new(TaskManagerInner {
                tasks: DashMap::new(),
                cancel_token,
                waiting: Mutex::new(None),
            }),
        }
    }

    /// 注册并启动一个后台任务，返回该任务的子取消令牌。
    ///
    /// 同名任务允许共存：每次注册都会追加到同名任务列表，互不覆盖。
    /// 取消语义为强中断：令牌取消时该任务立即被终止（适用于一次性/短任务）。
    /// 若需要“跑完当前这轮再退出”的长任务，请使用 [`Self::spawn_schedule`] 等。
    pub fn spawn<F>(&self, name: impl Into<String>, future: F) -> CancellationToken
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let name = name.into();
        let child_token = self.inner.cancel_token.child_token();
        let token_clone = child_token.clone();
        let task_name = name.clone();

        debug!("启动后台任务: {}", task_name);

        let handle = tokio::spawn(async move {
            tokio::select! {
                // biased：若任务恰好在取消同一瞬间自然完成，优先记为正常完成
                biased;
                _ = future => {
                    debug!("任务 '{}' 正常完成", task_name);
                }
                _ = token_clone.cancelled() => {
                    debug!("任务 '{}' 被取消", task_name);
                }
            }
        });

        self.push_handle(name, handle);

        child_token
    }

    /// 注册一个按调度触发的常驻任务，返回该任务的子取消令牌。
    ///
    /// 取消语义：**等待阶段**响应取消立即退出；**执行阶段**（`task()`）不参与
    /// 取消竞速，不会被中断，会跑完当前这一轮，随后由下一次循环感知取消退出。
    /// 因此适合备份这类“单轮执行较久、需要收尾完整性”的任务。
    pub fn spawn_schedule<F, Fut>(
        &self,
        name: impl Into<String>,
        schedule: TaskSchedule,
        task: F,
    ) -> CancellationToken
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let name = name.into();
        let child_token = self.inner.cancel_token.child_token();
        let token_clone = child_token.clone();
        let task_name = name.clone();
        let trace_string = format!("定时任务 '{}' 触发", name);

        debug!("启动定时任务: {} 于 {:?}", name, schedule);

        let handle = tokio::spawn(async move {
            loop {
                // 等待到下一个触发点：此阶段响应取消，可随时退出
                let delay_secs = delay_until_next(schedule);
                tokio::select! {
                    biased;
                    _ = tokio::time::sleep(delay_secs) => {}
                    _ = token_clone.cancelled() => {
                        debug!("定时任务 '{}' 被取消", task_name);
                        return;
                    }
                }
                trace!("{}", trace_string);
                // 执行阶段不参与取消竞速：取消不会打断当前这一轮
                task().await;
            }
        });

        self.push_handle(name, handle);

        child_token
    }

    /// 注册一个按固定间隔触发的常驻任务，返回该任务的子取消令牌。
    ///
    /// 取消语义同 [`Self::spawn_schedule`]：等待阶段可取消，执行阶段跑完当前轮。
    pub fn spawn_interval<F, Fut>(
        &self,
        name: impl Into<String>,
        interval: Duration,
        task: F,
    ) -> CancellationToken
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let name = name.into();
        let child_token = self.inner.cancel_token.child_token();
        let token_clone = child_token.clone();
        let task_name = name.clone();
        let trace_string = format!("间隔任务 '{}' 触发", name);

        debug!("启动间隔任务: {} 每 {}s", name, interval.as_secs());

        let handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    biased;
                    _ = interval_timer.tick() => {}
                    _ = token_clone.cancelled() => {
                        debug!("间隔任务 '{}' 被取消", task_name);
                        return;
                    }
                }
                trace!("{}", trace_string);
                task().await;
            }
        });

        self.push_handle(name, handle);

        child_token
    }

    pub fn cancel_all(&self) {
        self.inner.cancel_token.cancel();
    }

    /// 取消所有任务并等待其完成。
    pub async fn shutdown(&self, timeout: Duration) -> bool {
        self.cancel_all();
        self.wait_for_all(timeout).await
    }

    pub async fn wait_for_all(&self, timeout: Duration) -> bool {
        if self.inner.tasks.is_empty() {
            return true;
        }

        match timeout {
            Duration::ZERO => {
                self.wait_all().await;
                info!("所有后台任务已完成");
                true
            }
            _ => match tokio::time::timeout(timeout, async { self.wait_all().await }).await {
                Ok(_) => {
                    info!("所有后台任务已完成");
                    true
                }
                Err(_) => {
                    error!(
                        "等待后台任务超时 ({}s)，仍在运行的任务: {:?}，强制退出",
                        timeout.as_secs(),
                        self.running_names()
                    );
                    false
                }
            },
        }
    }

    /// 根取消令牌。
    pub fn cancel_token(&self) -> CancellationToken {
        self.inner.cancel_token.clone()
    }

    async fn wait_all(&self) {
        loop {
            // 取一个待处理的任务组名
            let group_name = {
                let entry_guard = self.inner.tasks.iter().next();
                let Some(entry) = entry_guard else {
                    break; // 没有更多任务
                };
                entry.key().clone()
            };

            let Some((name, entries)) = self.inner.tasks.remove(&group_name) else {
                // 可能在迭代过程中被其他线程移除，继续循环
                tokio::task::yield_now().await;
                continue;
            };

            // 记录当前正在等待的组，便于超时时报告仍在运行的任务
            *self.inner.waiting.lock().unwrap() = Some(name.clone());
            for task in entries {
                let _ = task.await;
            }
            *self.inner.waiting.lock().unwrap() = None;
            debug!("任务组 '{}' 已完成", name);
        }
    }

    /// 收集仍在运行的任务名（未处理的任务组 + 当前正在等待的任务组）。
    pub fn running_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.inner.tasks.iter().map(|e| e.key().clone()).collect();
        if let Some(waiting) = self.inner.waiting.lock().unwrap().as_ref()
            && !names.iter().any(|n| n == waiting)
        {
            names.push(waiting.clone());
        }
        names
    }

    /// 将任务句柄追加到同名任务列表。
    fn push_handle(&self, name: String, handle: JoinHandle<()>) {
        self.inner.tasks.entry(name).or_default().push(handle);
    }
}

/// 计算距离下一个调度触发点的秒数（本地时间）。
fn delay_until_next(schedule: TaskSchedule) -> Duration {
    let now = Local::now();
    let next = match schedule {
        TaskSchedule::Daily { hour, minute } => {
            let candidate = now
                .date_naive()
                .and_hms_opt(hour, minute, 0)
                .and_then(|naive| naive.and_local_timezone(Local).earliest())
                .unwrap_or_else(|| panic!("无效的调度时刻: {hour:02}:{minute:02}"));
            if candidate > now {
                candidate
            } else {
                candidate + chrono::Duration::days(1)
            }
        }
    };

    let diff = next - now;
    diff.to_std().unwrap_or(Duration::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    /// 执行阶段收到取消不应被打断，应跑完当前这一轮。
    #[tokio::test]
    async fn scheduled_execution_not_interrupted_by_cancel() {
        let tm = TaskManager::new();
        let started = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));

        tm.spawn_interval("interval-test", Duration::from_millis(20), {
            let started = started.clone();
            let finished = finished.clone();
            move || {
                let started = started.clone();
                let finished = finished.clone();
                async move {
                    // 仅第一轮执行长任务，避免后续轮次干扰
                    if started.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    finished.store(true, Ordering::SeqCst);
                }
            }
        });

        // 等待任务进入执行阶段
        while !started.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }

        // 执行中途取消：当前这一轮必须跑完
        tm.cancel_all();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(finished.load(Ordering::SeqCst), "执行阶段被取消打断了");
    }

    /// 等待阶段收到取消应立即退出，且后续不再触发执行。
    #[tokio::test]
    async fn waiting_task_exits_promptly_on_cancel() {
        let tm = TaskManager::new();
        let count = Arc::new(AtomicU32::new(0));

        tm.spawn_interval("interval-test-2", Duration::from_secs(3600), {
            let count = count.clone();
            move || {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                }
            }
        });

        // 间隔任务首次 tick 会立即执行一次，等它执行完并进入等待阶段
        while count.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(tm.shutdown(Duration::from_secs(2)).await);
        // 取消后不再触发新一轮执行
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(count.load(Ordering::SeqCst), 1, "取消后仍触发了新一轮执行");
    }

    /// 等待超时时，应能报告仍在运行的任务名。
    #[tokio::test]
    async fn shutdown_timeout_reports_running_task() {
        let tm = TaskManager::new();
        tm.spawn_interval("long-running", Duration::from_millis(10), || async move {
            // 长执行体：取消不会打断它，用于模拟超时
            tokio::time::sleep(Duration::from_secs(30)).await;
        });

        // 等待任务进入执行阶段（首次 tick 立即触发）
        tokio::time::sleep(Duration::from_millis(50)).await;

        let finished = tm.shutdown(Duration::from_millis(100)).await;
        assert!(!finished, "长任务未退出，应判定超时");
        assert_eq!(tm.running_names(), vec!["long-running"]);
    }
}
