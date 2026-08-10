use common::error::AppError;
use std::time::Duration;

/// 通用 mock loader:将请求的 ids 映射为数据,零 IO
///
/// 用于模拟"数据库命中返回"的 loader,不引入任何 IO 延迟,
/// 便于衡量缓存层自身的开销。
pub async fn loader<K, T>(ids: Vec<K>, make: impl Fn(K) -> T) -> Result<Vec<T>, AppError> {
    Ok(ids.into_iter().map(make).collect())
}

/// 带固定 IO 延迟的 mock loader,模拟真实数据库查询耗时
pub async fn delayed_loader<K, T>(
    delay: Duration,
    ids: Vec<K>,
    make: impl Fn(K) -> T,
) -> Result<Vec<T>, AppError> {
    tokio::time::sleep(delay).await;
    loader(ids, make).await
}
