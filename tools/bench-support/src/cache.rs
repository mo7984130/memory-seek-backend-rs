use std::time::Duration;

use common::error::AppError;
use multi_level_cache::{CacheConfig, MultiLevelCache, backend::Backend};

/// 缓存实例的 L1 TTL(1 小时),基准过程中足够避免过期
const LOCAL_TTL: Duration = Duration::from_secs(3600);

/// 构建多级缓存实例
///
/// 统一 L1 容量开关与 TTL,bench 场景只关心缓存名与启停。
/// `backend` 传 [`InMemoryBackend`] 时为纯内存后端(零 IO,消除测量波动)。
pub fn make_cache<T>(
    name: &'static str,
    backend: impl Backend,
    enabled: bool,
    local_capacity: u64,
) -> MultiLevelCache<T, AppError>
where
    T: Clone + serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
{
    MultiLevelCache::new_with_name(
        name,
        backend,
        CacheConfig::new(enabled, local_capacity, LOCAL_TTL),
    )
}
