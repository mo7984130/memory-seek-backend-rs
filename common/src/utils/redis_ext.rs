use crate::error::AppError;
use crate::utils::result_ext::ResultExt;
use deadpool_redis::{Connection, Pool};
use redis::{AsyncCommands, FromRedisValue, ToSingleRedisArg};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::future::Future;
use tracing::{error, warn};

pub trait RedisExt {
    fn get_conn(&self) -> impl Future<Output = Result<Connection, AppError>> + Send;

    fn get_as<T: FromRedisValue + Send + Sync>(&self, key: impl AsRef<str> + Send + Sync) -> impl Future<Output = Result<Option<T>, AppError>> + Send;

    fn set_ex<T: ToSingleRedisArg + Send + Sync>(&self, key: impl AsRef<str> + Send + Sync, value: T, ttl: u64) -> impl Future<Output = Result<(), AppError>> + Send;

    fn delete(&self, key: impl AsRef<str> + Send + Sync) -> impl Future<Output = Result<(), AppError>> + Send;
}

impl RedisExt for Pool {
    #[inline]
    async fn get_conn(&self) -> Result<Connection, AppError> {
        self.get().await.map_internal_err("redis 连接获取错误")
    }

    #[inline]
    async fn get_as<T: FromRedisValue + Send + Sync>(&self, key: impl AsRef<str> + Send + Sync) -> Result<Option<T>, AppError> {
        let mut conn = self.get_conn().await?;
        let result: Option<T> = conn.get(key.as_ref()).await.map_internal_err("redis 获取错误")?;
        Ok(result)
    }

    #[inline]
    async fn set_ex<T: ToSingleRedisArg + Send + Sync>(&self, key: impl AsRef<str> + Send + Sync, value: T, ttl: u64) -> Result<(), AppError> {
        let mut conn = self.get_conn().await?;
        conn.set_ex(key.as_ref(), value, ttl).await.map_internal_err("Redis 写入失败")
    }

    #[inline]
    async fn delete(&self, key: impl AsRef<str> + Send + Sync) -> Result<(), AppError> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.del(key.as_ref()).await.map_internal_err("redis删除key错误")?;
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
    async fn get_or_load<T, F, Fut>(
        &self,
        key: String,
        ttl: u64,
        loader: F,
    ) -> Result<T, AppError>
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
                    warn!("get_or_load中 逆序列化存储在redis中的数据时错误key {}: {:?}", key, e);
                }
            }
        }

        let value = loader().await?;
        if let Ok(json) = serde_json::to_string(&value) {
            self.set_ex(&key, json, ttl).await
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
        param_extractor: M
    ) -> Result<Vec<Option<V>>, AppError>
    where
        K: Clone + Serialize + Send + Sync + std::hash::Hash + Eq + Debug,
        V: Clone + Serialize + DeserializeOwned + Send + Sync + Debug,
        F: FnOnce(Vec<K>) -> Fut + Send,
        Fut: Future<Output = Result<Vec<V>, AppError>> + Send,
        M: Fn(&V) -> K + Send + Sync
    {
        if params.is_empty() { return Ok(vec![]); }

        let params_len = params.len();
        let mut conn = self.get().await.map_internal_err("Redis连接获取失败")?;

        // 建立 ID 到 原始索引 的映射
        let mut param_to_index = std::collections::HashMap::with_capacity(params.len());
        let mut keys = Vec::with_capacity(params.len());

        for (i, p) in params.iter().enumerate() {
            keys.push(key_provider(&p));

            param_to_index.insert(p.clone(), i);
        }

        // MGET 批量获取
        let cached_jsons: Vec<Option<String>> = conn.mget(&keys[..]).await.unwrap_or_else(|_| {
            error!("get_or_load_batch 批量获取失败 keys {:?}", &keys);
            vec![None; keys.len()]
        });

        let mut final_results: Vec<Option<V>> = vec![None; params_len];
        let mut miss_params = Vec::new();

        // 解析缓存
        for (i, json_opt) in cached_jsons.into_iter().enumerate() {
            if let Some(json) = json_opt {
                if let Ok(val) = serde_json::from_str::<V>(&json) {
                    final_results[i] = Some(val);
                    continue;
                }
            }
            miss_params.push(params[i].clone());
        }

        // 加载缺失数据并回写
        if !miss_params.is_empty() {
            let fresh_data = loader(miss_params).await?;

            let mut pipe = redis::pipe();
            let mut has_update = false;

            for item in fresh_data {
                let param = param_extractor(&item);
                if let Some(&original_idx) = param_to_index.get(&param) {
                    if let Ok(json) = serde_json::to_string(&item) {
                        pipe.set_ex(&keys[original_idx], json, ttl).ignore();
                        has_update = true;
                    }

                    final_results[original_idx] = Some(item);
                }
            }

            if has_update {
                let _ : () = pipe.query_async(&mut conn).await.unwrap_or_else(|e| {
                    warn!("get_or_load_batch中 Redis Pipeline 批量回写失败: {:?}", e);
                    ()
                });
            }
        }

        Ok(final_results)
    }
}
