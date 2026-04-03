use std::future::Future;
use crate::utils::MetricsTimer;

pub trait MetricsTimerExt: Future + Sized {
    fn timed(self, name: &'static str) -> impl Future<Output = Self::Output> + Send
    where
        Self: Send
    {
        async move {
            let _timer = MetricsTimer::start(name);
            self.await
        }
    }
}

// 为所有实现了 Future 的类型自动挂载此方法
impl<F: Future> MetricsTimerExt for F {}