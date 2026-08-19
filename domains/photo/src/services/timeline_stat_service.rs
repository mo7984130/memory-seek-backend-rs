use std::sync::Arc;

use crate::{
    repo::TimelineStatRepo,
    services::photo_service::{AfterPhotoDelete, AfterPhotoUpload},
    state::PhotoState,
};
use common::Result;
use types::photo::dto::timeline_stat::MonthStat;

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    /// 查询按月份聚合的照片时间线统计.
    #[common::metered]
    #[tracing::instrument(skip_all)]
    pub async fn get_monthly_stats(state: &PhotoState) -> Result<Vec<MonthStat>> {
        // 带三级缓存的月度统计（整表一条聚合）
        let stats = TimelineStatRepo::get_monthly_stats(state).await?;

        Ok(stats)
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::PhotoState,
    event = crate::services::photo_service::AfterPhotoDelete,
    slice = crate::services::photo_service::AFTER_PHOTO_DELETE_CONSUMERS,
    name = "timeline_stat_cache_invalidation",
)]
impl TimelineStatService {
    /// 删除照片后失效月度统计缓存。
    async fn on_after_photo_delete(
        &self,
        state: Arc<PhotoState>,
        _event: Arc<AfterPhotoDelete>,
    ) -> common::Result<()> {
        TimelineStatRepo::invalidate_cache(&state).await;
        Ok(())
    }
}

// 照片删除步骤:时间线统计清理
#[step_derive::declare_transaction_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "timeline_stat_cleanup",
    owns = ["TimelineStatMapper"],
)]
impl TimelineStatService {
    /// 删除照片后扣减对应月份的时间线统计.
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::Result<()> {
        let created_ats = ctx.photos.iter().map(|p| &p.created_at).collect::<Vec<_>>();
        crate::repo::TimelineStatRepo::decrement_by_created_ats(txn, &created_ats).await?;
        Ok(())
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::PhotoState,
    event = crate::services::photo_service::AfterPhotoUpload,
    slice = crate::services::photo_service::AFTER_PHOTO_UPLOAD_CONSUMERS,
    name = "timeline_stat_create",
)]
impl TimelineStatService {
    /// 上传照片后增加对应月份的时间线统计.
    async fn on_after_photo_upload(
        &self,
        state: Arc<PhotoState>,
        event: Arc<AfterPhotoUpload>,
    ) -> common::Result<()> {
        TimelineStatRepo::record_uploaded_photo(&state, event.photo.created_at).await?;
        Ok(())
    }
}
