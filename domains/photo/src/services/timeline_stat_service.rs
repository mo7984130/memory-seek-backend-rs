use std::sync::Arc;

use crate::{services::photo_service::AfterPhotoUpload, state::PhotoState};
use common::Result;
use types::photo::dto::timeline_stat::MonthStat;

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    /// 查询按月份聚合的照片时间线统计.
    #[common::metered]
    #[tracing::instrument(skip_all)]
    pub async fn get_monthly_stats(state: &PhotoState) -> Result<Vec<MonthStat>> {
        // 带三级缓存的月度统计（整表一条聚合）
        let stats = state.timeline_stat_repo.get_monthly_stats().await?;

        Ok(stats)
    }
}

// 照片删除步骤:时间线统计清理
#[step_derive::declare_step(
    ctx = crate::repo::photo_repo::PhotoDeleteContext,
    slice = crate::repo::photo_repo::PHOTO_DELETE_STEPS,
    name = "timeline_stat_cleanup",
    owns = ["TimelineStatMapper"],
)]
impl TimelineStatService {
    /// 删除照片后扣减对应月份的时间线统计.
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::repo::photo_repo::PhotoDeleteContext,
    ) -> common::Result<()> {
        let created_ats = ctx.photos.iter().map(|p| &p.created_at).collect::<Vec<_>>();
        crate::repo::TimelineStatRepo::decrement_by_created_ats(txn, &created_ats).await?;
        Ok(())
    }
}

#[step_derive::declare_event_handler(
    state = crate::state::PhotoState,
    event = crate::services::photo_service::AfterPhotoUpload,
    slice = crate::services::photo_service::AFTER_PHOTO_UPLOAD_HANDLERS,
    name = "timeline_stat_create",
)]
impl TimelineStatService {
    /// 上传照片后增加对应月份的时间线统计.
    async fn on_after_photo_upload(
        &self,
        state: Arc<PhotoState>,
        event: Arc<AfterPhotoUpload>,
    ) -> common::Result<()> {
        state
            .timeline_stat_repo
            .record_uploaded_photo(event.photo.created_at)
            .await?;
        Ok(())
    }
}
