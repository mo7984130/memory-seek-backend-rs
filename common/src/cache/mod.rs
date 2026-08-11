//! 多级缓存组件
//!
//! 统一项目中读多写少热点数据的缓存使用，分为三级：
//! - L1: 进程内 moka 缓存（直接存储反序列化对象，命中零拷贝）
//! - L2: Redis 分布式缓存（JSON 序列化存储）
//! - L3: 数据库（最终数据源，由调用方提供的 loader 承担）
//!
//! 写操作采用「覆盖 + 删除」策略：更新数据时通过 [`MultiLevelCache::put`] 直接覆盖
//! L1 与 L2，避免先删后写带来的穿透窗口；删除数据时通过 [`MultiLevelCache::invalidate`]
//! 同时清理 L1 与 L2。
//!
//! 本地缓存（L1）仅依赖短 TTL 与主动失效保证最终一致，多实例场景下不做跨实例广播。
//! 通过 [`CacheConfig::enabled`] 可整体禁用缓存（读写全部穿透数据库），便于压测对比。
//!
//! # L2 后端抽象
//!
//! L2 通过 [`CacheBackend`] 抽象：生产环境由 [`deadpool_redis::Pool`] 实现（真实 Redis），
//! 基准测试可提供内存 mock 实现，消除真实 IO 对测量的影响。
//!
//! # 监控
//!
//! 启用 `metrics` feature 时，每个缓存实例按 `cache:{name}:{layer}:{op}` 命名采集
//! 命中率、耗时与容量指标。

mod multi_level;

pub use multi_level::{CacheConfig, MultiLevelCache};

use async_trait::async_trait;
use deadpool_redis::Pool;
use redis::AsyncCommands;

use crate::error::AppError;
use crate::ext::TraceExt;

/// 缓存 L2 后端的抽象接口
///
/// 生产环境由 [`deadpool_redis::Pool`] 实现（真实 Redis）；基准测试可用内存 mock
/// 实现，从而消除真实网络 IO 对基准测量的波动。
#[async_trait]
pub trait CacheBackend: Send + Sync + 'static {
    /// 读取单个 key，`None` 表示不存在
    async fn get(&self, key: &str) -> Result<Option<String>, AppError>;

    /// 批量读取多个 key，结果与 `keys` 等长对齐
    async fn get_many(&self, keys: &[String]) -> Result<Vec<Option<String>>, AppError>;

    /// 写入单个 key 并设置过期时间（秒）
    async fn set_ex(&self, key: &str, value: String, ttl_secs: u64) -> Result<(), AppError>;

    /// 批量写入并设置过期时间（秒）
    async fn set_ex_many(&self, items: &[(String, String, u64)]) -> Result<(), AppError>;

    /// 删除单个 key
    async fn del(&self, key: &str) -> Result<(), AppError>;

    /// 批量删除多个 key
    async fn del_many(&self, keys: &[String]) -> Result<(), AppError>;
}

#[async_trait]
impl CacheBackend for Pool {
    async fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        let mut conn = self.get().await.trace()?;
        let value: Option<String> = conn.get(key).await.trace()?;
        Ok(value)
    }

    async fn get_many(&self, keys: &[String]) -> Result<Vec<Option<String>>, AppError> {
        let mut conn = self.get().await.trace()?;
        let values: Vec<Option<String>> = conn.mget(keys).await.trace()?;
        Ok(values)
    }

    async fn set_ex(&self, key: &str, value: String, ttl_secs: u64) -> Result<(), AppError> {
        let mut conn = self.get().await.trace()?;
        let _: () = conn.set_ex(key, value, ttl_secs).await.trace()?;
        Ok(())
    }

    async fn set_ex_many(&self, items: &[(String, String, u64)]) -> Result<(), AppError> {
        let mut conn = self.get().await.trace()?;
        let mut pipe = redis::pipe();
        for (key, value, ttl_secs) in items {
            pipe.set_ex(key, value, *ttl_secs).ignore();
        }
        let _: () = pipe.query_async(&mut conn).await.trace()?;
        Ok(())
    }

    async fn del(&self, key: &str) -> Result<(), AppError> {
        let mut conn = self.get().await.trace()?;
        let _: usize = conn.del(key).await.trace()?;
        Ok(())
    }

    async fn del_many(&self, keys: &[String]) -> Result<(), AppError> {
        let mut conn = self.get().await.trace()?;
        let _: usize = conn.del(keys).await.trace()?;
        Ok(())
    }
}
