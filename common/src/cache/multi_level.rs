//! 三级缓存组件实现
//!
//! 读取顺序：L1（进程内 moka）→ L2（Redis）→ L3（数据库 loader），逐级回填。
//! 写入策略：更新走 `put` 直接覆盖 L1+L2，删除走 `invalidate` 同时清理 L1+L2。

use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use deadpool_redis::Pool;
use indexmap::IndexMap;
use moka::future::Cache;
use redis::AsyncCommands;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::warn;

use crate::error::AppError;
use crate::ext::{RedisExt, TraceExt};

/// 多级缓存组件
///
/// 泛型参数 `T` 为缓存的数据类型，一个实例只缓存一种数据类型。
/// 不同数据类型各自持有独立的 [`MultiLevelCache`] 实例（如用户信息、照片信息）。
pub struct MultiLevelCache<T> {
    /// 缓存实例名称，用于指标命名前缀 `cache:{name}:*`
    #[allow(dead_code)]
    name: &'static str,
    /// L1 进程内缓存，直接存储反序列化对象，命中零拷贝
    local: Cache<String, Arc<T>>,
    /// L2 Redis 连接池，以 JSON 字符串存储
    redis: Pool,
}

impl<T> MultiLevelCache<T>
where
    T: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// 创建多级缓存实例
    ///
    /// # 参数
    /// - `name`: 缓存实例名称（用于指标前缀与日志，如 `user_info`）
    /// - `redis`: Redis 连接池（L2）
    /// - `local_capacity`: L1 最大条目数
    /// - `local_ttl`: L1 TTL（短 TTL，保证多实例下最终一致）
    pub fn new(name: &'static str, redis: Pool, local_capacity: u64, local_ttl: Duration) -> Self {
        let local = Cache::builder()
            .max_capacity(local_capacity)
            .time_to_live(local_ttl)
            .build();
        Self { name, local, redis }
    }

    /// 获取缓存，未命中时依次查找 L2、调用 loader 加载并逐级回填
    ///
    /// # 参数
    /// - `key`: 缓存 key
    /// - `ttl`: L2（Redis）过期时间（秒）
    /// - `loader`: L3 数据库加载闭包，仅当 L1/L2 均未命中时调用
    pub async fn get_or_load<F, Fut>(&self, key: String, ttl: u64, loader: F) -> Result<T, AppError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, AppError>> + Send,
    {
        // ---- L1 命中 ----
        let start = Instant::now();
        if let Some(value) = self.local.get(&key).await {
            self.record_duration("l1:get:duration_seconds", start);
            self.inc_counter("l1:hits", 1);
            return Ok((*value).clone());
        }
        self.record_duration("l1:get:duration_seconds", start);
        self.inc_counter("l1:misses", 1);

        // ---- L2 命中 ----
        let start = Instant::now();
        let cached: Option<String> = self.redis.get_as(&key).await?;
        if let Some(json) = cached {
            match serde_json::from_str::<T>(&json) {
                Ok(value) => {
                    self.record_duration("l2:get:duration_seconds", start);
                    self.inc_counter("l2:hits", 1);
                    self.write_l1(key, value.clone()).await;
                    return Ok(value);
                }
                Err(e) => {
                    warn!("MultiLevelCache 反序列化 L2 缓存失败 key={}: {:?}", key, e);
                }
            }
        }
        self.record_duration("l2:get:duration_seconds", start);
        self.inc_counter("l2:misses", 1);

        // ---- L3 数据库加载 ----
        let start = Instant::now();
        let value = loader().await?;
        self.record_duration("db:load:duration_seconds", start);
        self.inc_counter("db:loads", 1);

        // 回填 L2 + L1
        if let Ok(json) = serde_json::to_string(&value) {
            self.redis
                .set_ex(&key, json, ttl)
                .await
                .unwrap_or_else(|e| warn!("MultiLevelCache 写 L2 失败 key={}: {:?}", key, e));
        } else {
            warn!("MultiLevelCache 序列化数据失败 key={}", key);
        }
        self.write_l1(key, value.clone()).await;

        Ok(value)
    }

    /// 批量获取缓存，未命中的项通过 loader 批量加载并回写
    ///
    /// 先在 L1 中逐个命中，剩余项使用 MGET 批量查询 L2，仍未命中部分调用 loader。
    /// 支持参数去重：相同 key 的多个参数只加载一次，并广播结果到所有对应索引。
    ///
    /// # 参数
    /// - `params`: 待查询的参数列表
    /// - `key_provider`: 参数到缓存 key 的映射函数
    /// - `ttl`: L2（Redis）过期时间（秒）
    /// - `loader`: L3 数据库批量加载闭包
    /// - `result_mapper`: 从结果值反向提取参数 key 的映射函数
    ///
    /// # 返回
    /// 返回与 `params` 等长的结果列表，缓存未命中且 loader 未返回的项为 `None`
    pub async fn get_or_load_batch<K, F, Fut, M>(
        &self,
        params: &[K],
        key_provider: impl Fn(&K) -> String + Send + Sync,
        ttl: u64,
        loader: F,
        result_mapper: M,
    ) -> Result<Vec<Option<T>>, AppError>
    where
        K: Clone + Send + Sync + std::hash::Hash + Eq + Debug,
        F: FnOnce(Vec<K>) -> Fut + Send,
        Fut: Future<Output = Result<Vec<T>, AppError>> + Send,
        M: Fn(&T) -> K + Send + Sync,
    {
        if params.is_empty() {
            return Ok(vec![]);
        }

        let mut results: Vec<Option<T>> = vec![None; params.len()];

        // ---- L1 层：逐个命中 ----
        let mut l1_miss: Vec<(usize, K)> = Vec::new();
        for (i, p) in params.iter().enumerate() {
            let key = key_provider(p);
            let start = Instant::now();
            if let Some(value) = self.local.get(&key).await {
                self.record_duration("l1:get:duration_seconds", start);
                self.inc_counter("l1:hits", 1);
                results[i] = Some((*value).clone());
            } else {
                self.record_duration("l1:get:duration_seconds", start);
                self.inc_counter("l1:misses", 1);
                l1_miss.push((i, p.clone()));
            }
        }

        if l1_miss.is_empty() {
            self.update_entries_gauge();
            return Ok(results);
        }

        // ---- L2 层：MGET 批量命中 ----
        // redis_key -> (param, [原始索引])
        let mut key_to_info: IndexMap<String, (K, Vec<usize>)> = IndexMap::new();
        for (i, p) in l1_miss {
            let key = key_provider(&p);
            let entry = key_to_info.entry(key).or_insert_with(|| (p, Vec::new()));
            entry.1.push(i);
        }

        let unique_keys: Vec<&str> = key_to_info.keys().map(|s| s.as_str()).collect();
        let start = Instant::now();
        let cached_jsons: Vec<Option<String>> = {
            let mut conn = self.redis.get().await?;
            conn.mget(&unique_keys).await.unwrap_or_else(|e| {
                warn!("MultiLevelCache MGET 失败，降级为全量加载: {:?}", e);
                vec![None; unique_keys.len()]
            })
        };
        self.record_duration("l2:get:duration_seconds", start);

        let mut l2_miss_keys: Vec<usize> = Vec::new();
        for (idx, (key, (_, orig_indices))) in key_to_info.iter().enumerate() {
            match cached_jsons.get(idx).and_then(|o| o.as_deref()) {
                Some(json) => match serde_json::from_str::<T>(json) {
                    Ok(value) => {
                        self.inc_counter("l2:hits", 1);
                        self.write_l1(key.clone(), value.clone()).await;
                        for &i in orig_indices {
                            results[i] = Some(value.clone());
                        }
                    }
                    Err(e) => {
                        warn!("MultiLevelCache 反序列化 L2 缓存失败 key={}: {:?}", key, e);
                        self.inc_counter("l2:misses", 1);
                        l2_miss_keys.push(idx);
                    }
                },
                None => {
                    self.inc_counter("l2:misses", 1);
                    l2_miss_keys.push(idx);
                }
            }
        }

        // ---- L3 层：批量加载并回写 ----
        if !l2_miss_keys.is_empty() {
            let miss_params: Vec<K> = l2_miss_keys
                .iter()
                .map(|&idx| key_to_info.get_index(idx).unwrap().1.0.clone())
                .collect();

            let start = Instant::now();
            let fresh_data = loader(miss_params).await?;
            self.record_duration("db:load:duration_seconds", start);
            self.inc_counter("db:loads", fresh_data.len() as u64);

            let mut conn = self.redis.get().await?;
            let mut pipe = redis::pipe();
            let mut has_update = false;

            for item in fresh_data {
                let param = result_mapper(&item);
                let key = key_provider(&param);

                match key_to_info.get(&key) {
                    Some((_, orig_indices)) => {
                        let json = serde_json::to_string(&item)?;
                        pipe.set_ex(&key, json, ttl).ignore();
                        has_update = true;

                        self.write_l1(key.clone(), item.clone()).await;
                        for &i in orig_indices {
                            results[i] = Some(item.clone());
                        }
                    }
                    None => {
                        warn!("MultiLevelCache loader 返回了未请求的 key={}", key);
                    }
                }
            }

            if has_update {
                let _: Result<(), AppError> = pipe.query_async(&mut conn).await.trace();
            }
        }

        self.update_entries_gauge();
        Ok(results)
    }

    /// 直接覆盖 L1 与 L2（用于写操作后更新缓存）
    ///
    /// 相比「先删后写」，覆盖避免了并发期间的穿透窗口，写后立即读也能拿到新值。
    pub async fn put(&self, key: &str, value: T, ttl: u64) -> Result<(), AppError> {
        // L1 覆盖（moka `insert` 会自动重置 TTL）
        self.write_l1(key.to_string(), value.clone()).await;

        // L2 覆盖
        if let Ok(json) = serde_json::to_string(&value) {
            self.redis
                .set_ex(key, json, ttl)
                .await
                .unwrap_or_else(|e| warn!("MultiLevelCache put 写 L2 失败 key={}: {:?}", key, e));
        } else {
            warn!("MultiLevelCache put 序列化失败 key={}", key);
        }

        self.update_entries_gauge();
        Ok(())
    }

    /// 删除单个 key 的缓存（L1 + L2）
    ///
    /// 用于数据删除场景，覆盖无法表达「数据已不存在」。
    pub async fn invalidate(&self, key: &str) -> Result<(), AppError> {
        self.local.invalidate(key).await;
        self.redis.del(key).await?;
        self.update_entries_gauge();
        Ok(())
    }

    /// 批量删除缓存（L1 + L2）
    pub async fn invalidate_batch(&self, keys: &[String]) -> Result<(), AppError> {
        if keys.is_empty() {
            return Ok(());
        }
        for key in keys {
            self.local.invalidate(key).await;
        }
        let mut conn = self.redis.get().await?;
        let _: () = conn.del(keys).await?;
        self.update_entries_gauge();
        Ok(())
    }

    /// 写入 L1，`Arc` 引用计数共享，避免整体拷贝
    async fn write_l1(&self, key: String, value: T) {
        self.local.insert(key, Arc::new(value)).await;
    }

    // ============ 指标埋点 ============

    #[cfg(feature = "metrics")]
    fn metric_name(&self, step: &str) -> String {
        format!("cache:{}:{}", self.name, step)
    }

    #[cfg(feature = "metrics")]
    fn inc_counter(&self, step: &str, value: u64) {
        metrics::counter!(self.metric_name(step)).increment(value);
    }

    #[cfg(feature = "metrics")]
    fn record_duration(&self, step: &str, start: Instant) {
        metrics::histogram!(self.metric_name(step)).record(start.elapsed().as_secs_f64());
    }

    #[cfg(feature = "metrics")]
    fn update_entries_gauge(&self) {
        metrics::gauge!(self.metric_name("l1:entries")).set(self.local.entry_count() as f64);
    }

    #[cfg(not(feature = "metrics"))]
    fn inc_counter(&self, _step: &str, _value: u64) {}

    #[cfg(not(feature = "metrics"))]
    fn record_duration(&self, _step: &str, _start: Instant) {}

    #[cfg(not(feature = "metrics"))]
    fn update_entries_gauge(&self) {}
}
