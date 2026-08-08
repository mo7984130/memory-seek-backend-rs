use crate::mappers::timeline_stat_mapper::TimelineStatMapper;
use crate::state::PhotoState;
use common::{Result, metrics_group, metrics_name, metrics_success, utils::MetricsTimerExt};
use types::photo::dto::timeline_stat::MonthStat;

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    #[tracing::instrument(skip_all)]
    pub async fn get_monthly_stats(state: &PhotoState) -> Result<Vec<MonthStat>> {
        metrics_group!();

        let res = TimelineStatMapper::query_monthly_stats(&state.db)
            .timed(metrics_name!("query_monthly_stats"))
            .await;

        metrics_success!();
        res
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
