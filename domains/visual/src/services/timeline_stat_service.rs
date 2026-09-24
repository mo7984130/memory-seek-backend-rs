use std::sync::Arc;

use crate::{
    repo::TimelineStatRepo, services::visual_service::AfterVisualUpload, state::VisualState,
};
use common::{Result, ext::ToOk};
use types_visual::dto::timeline_stat::MonthStat;

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    /// 获取时间线统计.
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn get_monthly_stats(state: &VisualState) -> Result<Vec<MonthStat>> {
        TimelineStatRepo::get_monthly_stats(state).await?.to_ok()
    }
}

// 影像删除时
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "timeline_stat_cleanup",
    owns = ["TimelineStatMapper"],
    method = on_visual_delete,
)]
impl TimelineStatService {
    /// 删除影像后扣减对应月份的时间线统计.
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::visual_service::VisualDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let created_ats = ctx
            .visuals
            .iter()
            .map(|p| &p.created_at)
            .collect::<Vec<_>>();
        crate::repo::TimelineStatRepo::decrement_by_created_ats(txn, &created_ats).await?;
        Ok(())
    }
}

// 在影像上传之后
// 添加时间线统计
#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualUpload,
    slice = crate::services::visual_service::AFTER_MEDIA_UPLOAD_CONSUMERS,
    name = "timeline_stat_create",
)]
impl TimelineStatService {
    #[tracing::instrument(name = "upload_visual", skip_all)]
    async fn on_after_visual_upload(
        &self,
        state: Arc<VisualState>,
        event: Arc<AfterVisualUpload>,
    ) -> common::Result<()> {
        TimelineStatRepo::record_uploaded_visual(&state, event.visual.created_at).await?;
        Ok(())
    }
}
