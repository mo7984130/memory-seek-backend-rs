/// 为失败结果执行异步补偿或清理操作。
pub trait ResultInspectErrAsync<T, E> {
    #[allow(async_fn_in_trait)]
    async fn inspect_err_async<F, Fut>(self, f: F) -> Result<T, E>
    where
        F: FnOnce(&E) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send;
}

impl<T: Send, E: Send> ResultInspectErrAsync<T, E> for Result<T, E> {
    async fn inspect_err_async<F, Fut>(self, f: F) -> Result<T, E>
    where
        F: FnOnce(&E) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send,
    {
        if let Err(error) = &self {
            f(error).await;
        }
        self
    }
}
