use crate::mappers::timeline_stat_mapper::TimelineStatMapper;
use crate::state::PhotoState;
use common::{Result, metrics_group, metrics_name, metrics_success, utils::MetricsTimerExt};
use constants::RedisKeys;
use std::time::Duration;
use types::photo::dto::timeline_stat::MonthStat;

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    #[tracing::instrument(skip_all)]
    pub async fn get_monthly_stats(state: &PhotoState) -> Result<Vec<MonthStat>> {
        metrics_group!();

        // 带三级缓存的月度统计（整表一条聚合）
        let stats = state
            .cache_timeline_stat
            .get_or_load(
                RedisKeys::photo::timeline_stat::monthly_stats(),
                Duration::from_secs(60 * 60),
                || {
                    Box::pin(
                        async move { TimelineStatMapper::query_monthly_stats(&state.db).await },
                    )
                },
            )
            .timed(metrics_name!("cache_get_or_load"))
            .await?;

        metrics_success!();
        Ok(stats)
    }
}

// 照片删除步骤:时间线统计清理
#[step_derive::declare_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "timeline_stat_cleanup",
    owns = ["TimelineStatMapper"],
)]
impl TimelineStatService {
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::Result<()> {
        let created_ats = ctx.photos.iter().map(|p| &p.created_at).collect::<Vec<_>>();
        TimelineStatMapper::decr_stat_by_created_ats(txn, &created_ats).await?;
        Ok(())
    }
}
