/// Future 耗时监控扩展 trait
use std::future::Future;

use crate::MetricsTimer;

/// 为所有 `Future` 类型提供 `.timed()` 方法，用于自动记录异步任务的执行耗时
///
/// 启用 `metrics` feature 时，会在 future 执行期间插入 `MetricsTimer`，
/// 执行完成后自动将耗时写入指定的 histogram 指标。
pub trait MetricsTimerExt: Future + Sized {
    /// 包装 future 并在执行期间自动将耗时记录到指定 histogram 指标
    #[inline]
    fn timed(self, name: impl Into<String> + Send) -> impl Future<Output = Self::Output> + Send
    where
        Self: Send,
    {
        let name = name.into();
        async move {
            let _timer = MetricsTimer::start(name);
            #[cfg(not(feature = "metrics"))]
            let _ = &name;
            self.await
        }
    }
}

// 为所有实现了 Future 的类型自动挂载此方法
impl<F: Future> MetricsTimerExt for F {}
