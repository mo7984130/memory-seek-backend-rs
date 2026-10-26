use std::future::Future;

pub trait ResultInspectErrAsync<T, E> {
    /// 若为 `Err` 则异步执行回调，返回原结果
    fn inspect_err_async<F, Fut>(self, f: F) -> impl Future<Output = Result<T, E>> + Send
    where
        F: FnOnce(&E) -> Fut + Send,
        Fut: Future<Output = ()> + Send;
}

impl<T: Send, E: Send + Sync> ResultInspectErrAsync<T, E> for Result<T, E> {
    /// 若为 `Err` 则异步执行回调，返回原结果
    #[inline]
    async fn inspect_err_async<F, Fut>(self, f: F) -> Result<T, E>
    where
        F: FnOnce(&E) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        if let Err(ref err) = self {
            f(err).await;
        }

        self
    }
}
