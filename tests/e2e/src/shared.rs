//! 场景间/任务间共享的测试状态。
//!
//! 只放「并发测试确实需要共享」的东西:
//! - 共享登录态(同一账号被多个并发任务使用);
//! - 可复用的 `Barrier`(同步起跑);
//! - 共享计数器(例如统计同一轮内成功/冲突次数)。
//!
//! 注意:裸用 `Barrier` 时,若某个任务提前退出,其余等待者会一直阻塞。场景侧统一
//! 走 [`SharedState::sync_start`],它在参与者凑不齐时超时放行并打一条告警。

use std::{any::Any, collections::HashMap, sync::Arc};

use tokio::sync::{Barrier, OnceCell};

/// 泛型共享槽:同一个 key 只初始化一次,多个并发任务拿到同一个 `Arc<T>`。
type SharedCell = Arc<OnceCell<Arc<dyn Any + Send + Sync>>>;

/// 起跑栅栏的兜底等待时长(见 [`SharedState::sync_start`])。
pub const RACE_START_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// 测试上下文中的共享状态。
#[derive(Default)]
pub struct SharedState {
    barriers: std::sync::Mutex<HashMap<(&'static str, usize), Arc<Barrier>>>,
    counters: std::sync::Mutex<HashMap<&'static str, u64>>,
    cells: std::sync::Mutex<HashMap<&'static str, SharedCell>>,
    strings: std::sync::Mutex<HashMap<&'static str, Vec<String>>>,
}

impl SharedState {
    /// 取得指定场景、指定参与者数量的 `Barrier`。
    ///
    /// 必须按场景区分: 不同场景的轮次结构不同, 共用同一个实例会让上一场景
    /// 残留的等待者影响下一场景的起跑对齐。
    ///
    /// 参与者必须在每一轮都调用 `wait()`,否则其余任务会永久阻塞
    /// (调用方应配合 [`Self::sync_start`] 的超时兜底)。
    pub fn barrier(&self, tag: &'static str, participants: usize) -> Arc<Barrier> {
        assert!(participants > 0, "barrier 参与者必须大于 0");
        let mut barriers = self.barriers.lock().expect("shared barriers poisoned");
        barriers
            .entry((tag, participants))
            .or_insert_with(|| Arc::new(Barrier::new(participants)))
            .clone()
    }

    /// 原子递增并返回新值。
    pub fn incr(&self, key: &'static str) -> u64 {
        let mut counters = self.counters.lock().expect("shared counters poisoned");
        let value = counters.entry(key).or_insert(0);
        *value += 1;
        *value
    }

    /// 读取当前值(不存在时为 0)。
    pub fn get(&self, key: &'static str) -> u64 {
        self.counters
            .lock()
            .expect("shared counters poisoned")
            .get(key)
            .copied()
            .unwrap_or(0)
    }

    /// 重置计数器。
    pub fn reset(&self, key: &'static str) {
        self.counters
            .lock()
            .expect("shared counters poisoned")
            .remove(key);
    }

    /// 同一个 key 只初始化一次;并发调用者拿到同一个值。
    ///
    /// 典型用途是共享账号只登录一次:
    /// ```ignore
    /// let session = ctx.shared.get_or_init("hot_racer", || login(ctx, account)).await;
    /// ```
    pub async fn get_or_init<T, F, Fut>(&self, key: &'static str, init: F) -> Arc<T>
    where
        T: Any + Send + Sync,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let cell = {
            let mut cells = self.cells.lock().expect("shared cells poisoned");
            cells
                .entry(key)
                .or_insert_with(|| Arc::new(OnceCell::new()))
                .clone()
        };
        let value = cell
            .get_or_init(|| async { Arc::new(init().await) as Arc<dyn Any + Send + Sync> })
            .await;
        Arc::downcast::<T>(value.clone()).expect("共享状态类型不匹配")
    }

    /// 并发起跑同步。
    ///
    /// 框架的 `times` 模式会把总轮次均摊到各任务, 轮次不足时会有任务直接拿到
    /// 0 轮、永远不会进入 `run`; 此时硬等 `Barrier` 会永久阻塞。这里加超时兜底:
    /// 参与者凑齐就精确对齐, 凑不齐则在超时后放行(各任务仍几乎同时起跑)。
    pub async fn sync_start(
        &self,
        tag: &'static str,
        participants: usize,
        wait: std::time::Duration,
    ) {
        if tokio::time::timeout(wait, self.barrier(tag, participants).wait())
            .await
            .is_err()
        {
            tracing::warn!(
                tag,
                participants,
                "起跑栅栏未凑齐参与者, 超时后继续(times 模式轮次不足时属正常)"
            );
        }
    }

    /// 读取已初始化的共享值;未初始化时返回 `None`(不触发初始化)。
    ///
    /// 供 `reconcile` 使用:对账阶段不应再构造业务数据,只读取并发阶段留下的结果。
    pub fn peek<T>(&self, key: &'static str) -> Option<Arc<T>>
    where
        T: Any + Send + Sync,
    {
        let cell = {
            let cells = self.cells.lock().expect("shared cells poisoned");
            cells.get(key)?.clone()
        };
        let value = cell.get()?.clone();
        Arc::downcast::<T>(value).ok()
    }

    /// 追加一条字符串记录(例如并发阶段每个任务返回的 token)。
    pub fn push(&self, key: &'static str, value: String) {
        self.strings
            .lock()
            .expect("shared strings poisoned")
            .entry(key)
            .or_default()
            .push(value);
    }

    /// 取出并清空某 key 的全部字符串记录。
    pub fn take(&self, key: &'static str) -> Vec<String> {
        self.strings
            .lock()
            .expect("shared strings poisoned")
            .remove(key)
            .unwrap_or_default()
    }
}
