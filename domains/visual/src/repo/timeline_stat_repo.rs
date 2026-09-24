use common::time::Duration;
use std::sync::Arc;

use common::error::contextual::ext::{ContextualResultExt, IntoContextualExt};
use common::{error::contextual::Result, metrics_name, time::DateTime, utils::MetricsTimerExt};
use constants::RedisKeys;
use sea_orm::DatabaseTransaction;
use types_visual::dto::timeline_stat::MonthStat;

use crate::mappers::timeline_stat_mapper::TimelineStatMapper;
use crate::services::visual_service::AfterVisualDelete;
use crate::state::VisualState;

const TIMELINE_STAT_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// 时间线统计仓储，统一封装统计表和月度统计缓存。
pub struct TimelineStatRepo;

impl TimelineStatRepo {
    /// 记录新上传影像对应月份的时间线统计。
    pub async fn record_uploaded_visual(state: &VisualState, created_at: DateTime) -> Result<()> {
        TimelineStatMapper::incr_stat(&state.db, created_at)
            .timed(metrics_name!("db_update"))
            .await?;
        Self::invalidate_cache(state).await;
        Ok(())
    }

    /// 扣减对应月份的统计。
    pub async fn decrement_by_created_ats(
        txn: &DatabaseTransaction,
        created_ats: &[&DateTime],
    ) -> Result<()> {
        TimelineStatMapper::decr_by_created_ats(txn, created_ats)
            .timed(metrics_name!("db_update"))
            .await
    }

    /// 获取统计。
    pub async fn get_monthly_stats(state: &VisualState) -> Result<Vec<MonthStat>> {
        state
            .cache_timeline_stat
            .get_or_load(
                RedisKeys::visual::timeline_stat::monthly_stats(),
                TIMELINE_STAT_CACHE_TTL,
                || async move { TimelineStatMapper::query_monthly_stats(&state.db).await },
            )
            .timed(metrics_name!("cache_get_or_load"))
            .await
    }

    /// 失效缓存.
    pub async fn invalidate_cache(state: &VisualState) {
        state
            .cache_timeline_stat
            .invalidate(RedisKeys::visual::timeline_stat::monthly_stats())
            .timed(metrics_name!("cache_invalidate"))
            .await
            .into_contextual()
            .emit_if_err();
    }
}

// 在影像删除之后
#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualDelete,
    slice = crate::services::visual_service::AFTER_MEDIA_DELETE_CONSUMERS,
    name = "timeline_stat_cache_invalidation",
)]
impl TimelineStatRepo {
    /// 删除影像后失效月度统计缓存。
    #[tracing::instrument(name = "delete_visuals", skip_all)]
    async fn on_after_visual_delete(
        &self,
        state: Arc<VisualState>,
        _event: Arc<AfterVisualDelete>,
    ) -> std::result::Result<(), common::error::AppError> {
        Self::invalidate_cache(&state).await;
        Ok(())
    }
}
