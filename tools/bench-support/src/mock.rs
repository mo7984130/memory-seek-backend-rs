use common::error::AppError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 通用 bench key 前缀
pub const KEY_PREFIX: &str = "bench";

/// 通用 mock 数据行,模拟从数据库加载的一条记录
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MockRow {
    pub id: u64,
    pub payload: u64,
}

impl MockRow {
    pub fn new(id: u64) -> Self {
        Self { id, payload: 0 }
    }
}

/// 缓存 key 生成:`{KEY_PREFIX}:{id}`
pub fn key_of(id: &u64) -> String {
    format!("{}:{}", KEY_PREFIX, id)
}

/// 从数据行反向提取 id
pub fn id_of(row: &MockRow) -> u64 {
    row.id
}

/// 通用 mock loader:将请求的 ids 映射为数据,零 IO
///
/// 用于模拟"数据库命中返回"的 loader,不引入任何 IO 延迟,
/// 便于衡量缓存层自身的开销。
pub async fn loader<K, T>(ids: Vec<K>, make: impl Fn(K) -> T) -> Result<Vec<T>, AppError> {
    Ok(ids.into_iter().map(make).collect())
}

/// 模拟数据库:每次查询固定 IO 延迟,返回 mock 数据
///
/// 用于替代真实数据库作为 loader 的数据源,`query` 先 sleep 固定延迟
/// 再返回数据,消除真实 DB 网络 IO 对基准测量的波动。
pub struct MockDb {
    delay: Duration,
}

impl MockDb {
    /// 创建固定查询延迟的模拟数据库
    pub fn new(delay: Duration) -> Self {
        Self { delay }
    }

    /// 模拟一次数据库批量查询:先 sleep `delay`,再返回映射后的数据
    pub async fn query<K, T>(
        &self,
        ids: Vec<K>,
        make: impl Fn(K) -> T,
    ) -> Result<Vec<T>, AppError> {
        tokio::time::sleep(self.delay).await;
        loader(ids, make).await
    }
}
