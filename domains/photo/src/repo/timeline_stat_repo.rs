use std::time::Duration;

use common::{
    DateTime, error::ContextualError, error::contextual::Result, metrics_name,
    utils::MetricsTimerExt,
};
use constants::RedisKeys;
use deadpool_redis::Pool;
use multi_level_cache::{CacheConfig, MultiLevelCache};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use types::photo::dto::timeline_stat::MonthStat;

use crate::mappers::timeline_stat_mapper::TimelineStatMapper;

const TIMELINE_STAT_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// 时间线统计仓储，统一封装统计表和月度统计缓存。
pub struct TimelineStatRepo {
    db: DatabaseConnection,
    cache: MultiLevelCache<Vec<MonthStat>, ContextualError>,
}

impl TimelineStatRepo {
    pub fn new(db: DatabaseConnection, redis: Pool, cache_config: CacheConfig) -> Self {
        Self {
            db,
            cache: MultiLevelCache::new_with_name("timeline_stat", redis, cache_config),
        }
    }

    /// 记录新上传照片对应月份的时间线统计。
    pub async fn record_uploaded_photo(&self, created_at: DateTime) -> Result<()> {
        TimelineStatMapper::incr_stat(&self.db, created_at).await?;
        self.invalidate_cache().await;
        Ok(())
    }

    /// 在照片删除事务中扣减对应月份的统计。
    pub async fn decrement_by_created_ats(
        txn: &DatabaseTransaction,
        created_ats: &[&DateTime],
    ) -> Result<()> {
        TimelineStatMapper::decr_stat_by_created_ats(txn, created_ats).await
    }

    /// 获取带缓存的月度照片统计。
    pub async fn get_monthly_stats(&self) -> Result<Vec<MonthStat>> {
        self.cache
            .get_or_load(
                RedisKeys::photo::timeline_stat::monthly_stats(),
                TIMELINE_STAT_CACHE_TTL,
                || async move { TimelineStatMapper::query_monthly_stats(&self.db).await },
            )
            .timed(metrics_name!("cache_get_or_load"))
            .await
    }

    /// 失效月度照片统计缓存；缓存错误不影响主流程。
    pub async fn invalidate_cache(&self) {
        let _ = self
            .cache
            .invalidate(RedisKeys::photo::timeline_stat::monthly_stats())
            .timed(metrics_name!("cache_invalidate"))
            .await;
    }
}
