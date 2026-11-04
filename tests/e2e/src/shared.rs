//! 场景间/任务间共享的测试状态。
//!
//! 只放「并发测试确实需要共享」的东西:
//! - 共享登录态(同一账号被多个并发任务使用);
//! - 可复用的 `Barrier`(同步起跑);
//! - 共享计数器(例如统计同一轮内成功/冲突次数)。
//!
//! 注意:这里的 `Barrier` 是不可取消的。若某个任务提前退出,其余等待者会一直阻塞,
//! 使用方应保证参与者数量固定且不会中途退出。

use std::{any::Any, collections::HashMap, sync::Arc};

use tokio::sync::{Barrier, OnceCell};

/// 泛型共享槽:同一个 key 只初始化一次,多个并发任务拿到同一个 `Arc<T>`。
type SharedCell = Arc<OnceCell<Arc<dyn Any + Send + Sync>>>;

/// 测试上下文中的共享状态。
#[derive(Default)]
pub struct SharedState {
    barriers: std::sync::Mutex<HashMap<usize, Arc<Barrier>>>,
    counters: std::sync::Mutex<HashMap<&'static str, u64>>,
    cells: std::sync::Mutex<HashMap<&'static str, SharedCell>>,
}

impl SharedState {
    /// 取得指定参与者数量的 `Barrier`(同数量复用同一个实例)。
    ///
    /// 参与者必须在每一轮都调用 `wait()`,否则其余任务会永久阻塞。
    pub fn barrier(&self, participants: usize) -> Arc<Barrier> {
        assert!(participants > 0, "barrier 参与者必须大于 0");
        let mut barriers = self.barriers.lock().expect("shared barriers poisoned");
        barriers
            .entry(participants)
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
}
