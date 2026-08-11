use async_trait::async_trait;
use common::cache::{CacheBackend, CacheConfig, MultiLevelCache};
use common::error::AppError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 缓存实例的 L1 TTL(1 小时),基准过程中足够避免过期
const LOCAL_TTL: Duration = Duration::from_secs(3600);

/// 构建多级缓存实例
///
/// 统一 L1 容量开关与 TTL,bench 场景只关心缓存名与启停。
/// `backend` 传 [`MockRedis`] 时为纯内存后端(零 IO,消除测量波动)。
pub fn make_cache<T>(
    name: &'static str,
    backend: impl CacheBackend,
    enabled: bool,
    local_capacity: u64,
) -> MultiLevelCache<T>
where
    T: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    MultiLevelCache::new(
        name,
        backend,
        CacheConfig::new(enabled, local_capacity, LOCAL_TTL),
    )
}

struct Entry {
    value: String,
    expires_at: Option<Instant>,
}

/// 内存 L2 缓存后端,实现 [`CacheBackend`],零 IO
///
/// 用于替代真实 Redis 作为基准的 L2 后端,消除网络 IO 对测量的波动。
pub struct MockRedis {
    store: Mutex<HashMap<String, Entry>>,
}

impl MockRedis {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for MockRedis {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CacheBackend for MockRedis {
    async fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        let now = Instant::now();
        let mut map = self.store.lock().unwrap();
        if let Some(entry) = map.get(key)
            && entry.expires_at.is_none_or(|t| now < t)
        {
            return Ok(Some(entry.value.clone()));
        }
        map.remove(key);
        Ok(None)
    }

    async fn get_many(&self, keys: &[String]) -> Result<Vec<Option<String>>, AppError> {
        let mut values = Vec::with_capacity(keys.len());
        for key in keys {
            values.push(self.get(key).await?);
        }
        Ok(values)
    }

    async fn set_ex(&self, key: &str, value: String, ttl_secs: u64) -> Result<(), AppError> {
        self.store.lock().unwrap().insert(
            key.to_string(),
            Entry {
                value,
                expires_at: Some(Instant::now() + Duration::from_secs(ttl_secs)),
            },
        );
        Ok(())
    }

    async fn set_ex_many(&self, items: &[(String, String, u64)]) -> Result<(), AppError> {
        let now = Instant::now();
        let mut map = self.store.lock().unwrap();
        for (key, value, ttl_secs) in items {
            map.insert(
                key.clone(),
                Entry {
                    value: value.clone(),
                    expires_at: Some(now + Duration::from_secs(*ttl_secs)),
                },
            );
        }
        Ok(())
    }

    async fn del(&self, key: &str) -> Result<(), AppError> {
        self.store.lock().unwrap().remove(key);
        Ok(())
    }

    async fn del_many(&self, keys: &[String]) -> Result<(), AppError> {
        let mut map = self.store.lock().unwrap();
        for key in keys {
            map.remove(key);
        }
        Ok(())
    }
}
