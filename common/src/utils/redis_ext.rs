use crate::error::AppError;
use crate::utils::result_ext::ResultExt;
use deadpool_redis::{Connection, Pool};
use indexmap::IndexMap;
use redis::{AsyncCommands, FromRedisValue, ToSingleRedisArg};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::future::Future;
use std::{collections::HashMap, fmt::Debug};
use tracing::{error, warn};

pub trait RedisExt {
    fn get_conn(&self) -> impl Future<Output = Result<Connection, AppError>> + Send;

    fn get_as<T: FromRedisValue + Send + Sync>(
        &self,
        key: impl AsRef<str> + Send + Sync,
    ) -> impl Future<Output = Result<Option<T>, AppError>> + Send;

    fn set_ex<T: ToSingleRedisArg + Send + Sync>(
        &self,
        key: impl AsRef<str> + Send + Sync,
        value: T,
        ttl: u64,
    ) -> impl Future<Output = Result<(), AppError>> + Send;

    fn delete(
        &self,
        key: impl AsRef<str> + Send + Sync,
    ) -> impl Future<Output = Result<(), AppError>> + Send;
}

impl RedisExt for Pool {
    #[inline]
    async fn get_conn(&self) -> Result<Connection, AppError> {
        self.get().await.map_internal_err("redis 连接获取错误")
    }

    #[inline]
    async fn get_as<T: FromRedisValue + Send + Sync>(
        &self,
        key: impl AsRef<str> + Send + Sync,
    ) -> Result<Option<T>, AppError> {
        let mut conn = self.get_conn().await?;
        let result: Option<T> = conn
            .get(key.as_ref())
            .await
            .map_internal_err("redis 获取错误")?;
        Ok(result)
    }

    #[inline]
    async fn set_ex<T: ToSingleRedisArg + Send + Sync>(
        &self,
        key: impl AsRef<str> + Send + Sync,
        value: T,
        ttl: u64,
    ) -> Result<(), AppError> {
        let mut conn = self.get_conn().await?;
        conn.set_ex(key.as_ref(), value, ttl)
            .await
            .map_internal_err("Redis 写入失败")
    }

    #[inline]
    async fn delete(&self, key: impl AsRef<str> + Send + Sync) -> Result<(), AppError> {
        let mut conn = self.get_conn().await?;
        let _: () = conn
            .del(key.as_ref())
            .await
            .map_internal_err("redis删除key错误")?;
        Ok(())
    }
}

pub trait CacheExtension {
    fn get_or_load<T, F, Fut>(
        &self,
        key: String,
        ttl: u64,
        loader: F,
    ) -> impl Future<Output = Result<T, AppError>> + Send
    where
        T: Serialize + DeserializeOwned + Debug + Send,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, AppError>> + Send;

    fn get_or_load_batch<K, V, F, Fut, M>(
        &self,
        params: &[K],
        key_provider: impl Fn(&K) -> String + Send + Sync,
        ttl: u64,
        loader: F,
        result_mapper: M,
    ) -> impl Future<Output = Result<Vec<Option<V>>, AppError>> + Send
    where
        K: Clone + Serialize + Send + Sync + std::hash::Hash + Eq + Debug,
        V: Clone + Serialize + DeserializeOwned + Send + Sync + Debug,
        F: FnOnce(Vec<K>) -> Fut + Send,
        Fut: Future<Output = Result<Vec<V>, AppError>> + Send,
        M: Fn(&V) -> K + Send + Sync;
}

impl CacheExtension for Pool {
    async fn get_or_load<T, F, Fut>(&self, key: String, ttl: u64, loader: F) -> Result<T, AppError>
    where
        T: Serialize + DeserializeOwned + Debug + Send,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, AppError>> + Send,
    {
        let cached_data: Option<String> = self.get_as(&key).await?;
        if let Some(json) = cached_data {
            match serde_json::from_str::<T>(&json) {
                Ok(val) => return Ok(val),
                Err(e) => {
                    warn!(
                        "get_or_load中 逆序列化存储在redis中的数据时错误key {}: {:?}",
                        key, e
                    );
                }
            }
        }

        let value = loader().await?;
        if let Ok(json) = serde_json::to_string(&value) {
            self.set_ex(&key, json, ttl)
                .await
                .unwrap_or_else(|e| warn!("Redis 设置值错误: {:?}", e));
        } else {
            warn!("get_or_load中 序列化数据错误 {:?}", &value);
        }

        Ok(value)
    }

    async fn get_or_load_batch<K, V, F, Fut, M>(
        &self,
        params: &[K],
        key_provider: impl Fn(&K) -> String + Send + Sync,
        ttl: u64,
        loader: F,
        param_extractor: M,
    ) -> Result<Vec<Option<V>>, AppError>
    where
        K: Clone + Serialize + Send + Sync + std::hash::Hash + Eq + Debug,
        V: Clone + Serialize + DeserializeOwned + Send + Sync + Debug,
        F: FnOnce(Vec<K>) -> Fut + Send,
        Fut: Future<Output = Result<Vec<V>, AppError>> + Send,
        M: Fn(&V) -> K + Send + Sync,
    {
        // 如果参数为空，直接返回空结果
        if params.is_empty() {
            return Ok(vec![]);
        }

        // redis_key -> (param, [原始索引])
        let mut key_to_info: IndexMap<String, (K, Vec<usize>)> = IndexMap::new();
        for (i, p) in params.iter().enumerate() {
            let k = key_provider(p);
            let entry = key_to_info
                .entry(k)
                .or_insert_with(|| (p.clone(), Vec::new()));
            entry.1.push(i);
        }

        // 提取唯一的 redis_key
        let unique_keys: Vec<&str> = key_to_info.keys().map(|s| s.as_str()).collect();

        // MGET 批量获取
        let cached_jsons: Vec<Option<String>> = {
            let mut conn = self
                .get()
                .await
                .trace_internal_err("get_redis_conn_err", "Redis连接获取失败")?;
            conn.mget(&unique_keys).await.unwrap_or_else(|e| {
                warn!("get_or_load_batch MGET 失败，降级为全量加载: {:?}", e);
                vec![None; unique_keys.len()]
            })
        };

        let mut final_results: Vec<Option<V>> = vec![None; params.len()];
        let mut miss_indices = Vec::new();

        // 解析缓存, 命中的话, 广播到所有对应的索引
        for (idx, (key, (_, orig_indices))) in key_to_info.iter().enumerate() {
            match cached_jsons.get(idx).and_then(|o| o.as_deref()) {
                Some(json) => match serde_json::from_str::<V>(json) {
                    Ok(val) => {
                        for &i in orig_indices {
                            final_results[i] = Some(val.clone());
                        }
                    }
                    Err(e) => {
                        warn!("get_or_load_batch 反序列化失败 key={}: {:?}", key, e);
                        miss_indices.push(idx);
                    }
                },
                None => {
                    miss_indices.push(idx);
                }
            }
        }

        // 加载缺失数据并回写
        if !miss_indices.is_empty() {
            let miss_params: Vec<K> = miss_indices
                .iter()
                .map(|&idx| key_to_info.get_index(idx).unwrap().1.0.clone())
                .collect();

            let fresh_data = loader(miss_params).await?;

            let mut conn = self
                .get()
                .await
                .trace_internal_err("get_redis_conn_err", "Redis连接获取失败(回写)")?;
            let mut pipe = redis::pipe();
            let mut has_update = false;

            for item in fresh_data {
                let param = param_extractor(&item);
                let key = key_provider(&param);

                match key_to_info.get(&key) {
                    Some((_, orig_indices)) => {
                        match serde_json::to_string(&item) {
                            Ok(json) => {
                                pipe.set_ex(&key, json, ttl).ignore();
                                has_update = true;
                            }
                            Err(e) => {
                                warn!("get_or_load_batch 序列化失败 key={}: {:?}", key, e);
                            }
                        }
                        for &i in orig_indices {
                            final_results[i] = Some(item.clone());
                        }
                    }
                    None => {
                        warn!("get_or_load_batch loader 返回了未请求的 key={}", key);
                    }
                }
            }

            if has_update {
                let _: () = pipe.query_async(&mut conn).await.unwrap_or_else(|e| {
                    warn!("get_or_load_batch Pipeline 回写失败: {:?}", e);
                });
            }
        }

        Ok(final_results)
    }
}
