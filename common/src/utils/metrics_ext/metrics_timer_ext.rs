use crate::utils::MetricsTimer;
/// Future 耗时监控扩展 trait
use std::future::Future;

/// 为所有 `Future` 类型提供 `.timed()` 方法，用于自动记录异步任务的执行耗时
///
/// 启用 `metrics` feature 时，会在 future 执行期间插入 `MetricsTimer`，
/// 执行完成后自动将耗时写入指定的 histogram 指标。
pub trait MetricsTimerExt: Future + Sized {
    /// 包装当前 future，在执行期间自动记录耗时到指定的 histogram 指标
    ///
    /// # 参数
    /// - `name`: metrics histogram 指标名称（静态字符串）
    ///
    /// # 返回
    /// 包装后的 future，输出类型与原 future 相同
    #[inline]
    fn timed(self, name: &'static str) -> impl Future<Output = Self::Output> + Send
    where
        Self: Send,
    {
        async move {
            #[cfg(feature = "metrics")]
            let _timer = MetricsTimer::start(name);
            self.await
        }
    }
}

// 为所有实现了 Future 的类型自动挂载此方法
impl<F: Future> MetricsTimerExt for F {}
