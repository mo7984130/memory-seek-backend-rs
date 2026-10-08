use crate::Pool;
use deadpool_redis::{Connection, PoolError};
use redis::{AsyncCommands, FromRedisValue, ToSingleRedisArg};
use std::future::Future;

/// Redis 连接池的基础操作扩展 trait
pub trait RedisExt {
    /// 从连接池获取一个 Redis 连接
    fn get_conn(&self) -> impl Future<Output = Result<Connection, PoolError>> + Send;

    /// 从 Redis 读取指定 key 并反序列化为目标类型；key 不存在时返回 `None`
    fn get_as<T: FromRedisValue + Send + Sync>(
        &self,
        key: impl AsRef<str> + Send + Sync,
    ) -> impl Future<Output = Result<Option<T>, PoolError>> + Send;

    /// 将值写入 Redis 并设置过期时间（秒）
    fn set_ex<T: ToSingleRedisArg + Send + Sync>(
        &self,
        key: impl AsRef<str> + Send + Sync,
        value: T,
        ttl: u64,
    ) -> impl Future<Output = Result<(), PoolError>> + Send;

    /// 删除 Redis 中指定 key
    fn del(
        &self,
        key: impl AsRef<str> + Send + Sync,
    ) -> impl Future<Output = Result<(), PoolError>> + Send;
}

impl RedisExt for Pool {
    /// 从连接池获取一个 Redis 连接
    #[inline]
    async fn get_conn(&self) -> Result<Connection, PoolError> {
        self.get().await
    }

    /// 从 Redis 读取指定 key 并反序列化为目标类型；key 不存在时返回 `None`
    #[inline]
    async fn get_as<T: FromRedisValue + Send + Sync>(
        &self,
        key: impl AsRef<str> + Send + Sync,
    ) -> Result<Option<T>, PoolError> {
        let mut conn = self.get_conn().await?;
        let result: Option<T> = conn.get(key.as_ref()).await?;
        Ok(result)
    }

    /// 将值写入 Redis 并设置过期时间（秒）
    #[inline]
    async fn set_ex<T: ToSingleRedisArg + Send + Sync>(
        &self,
        key: impl AsRef<str> + Send + Sync,
        value: T,
        ttl: u64,
    ) -> Result<(), PoolError> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.set_ex(key.as_ref(), value, ttl).await?;
        Ok(())
    }

    /// 删除 Redis 中指定 key
    #[inline]
    async fn del(&self, key: impl AsRef<str> + Send + Sync) -> Result<(), PoolError> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.del(key.as_ref()).await?;
        Ok(())
    }
}
