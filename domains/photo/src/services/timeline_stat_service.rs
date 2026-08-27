use std::sync::Arc;

use crate::{repo::TimelineStatRepo, services::photo_service::AfterPhotoUpload, state::PhotoState};
use common::{Result, ext::ToOk};
use types::photo::dto::timeline_stat::MonthStat;

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    /// 获取时间线统计.
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn get_monthly_stats(state: &PhotoState) -> Result<Vec<MonthStat>> {
        TimelineStatRepo::get_monthly_stats(state).await?.to_ok()
    }
}

// 照片删除时
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
    ) -> common::error::contextual::Result<()> {
        let created_ats = ctx.photos.iter().map(|p| &p.created_at).collect::<Vec<_>>();
        crate::repo::TimelineStatRepo::decrement_by_created_ats(txn, &created_ats).await?;
        Ok(())
    }
}

// 在照片上传之后
// 添加时间线统计
#[step_derive::declare_event_consumer(
    state = crate::state::PhotoState,
    event = crate::services::photo_service::AfterPhotoUpload,
    slice = crate::services::photo_service::AFTER_PHOTO_UPLOAD_CONSUMERS,
    name = "timeline_stat_create",
)]
impl TimelineStatService {
    async fn on_after_photo_upload(
        &self,
        state: Arc<PhotoState>,
        event: Arc<AfterPhotoUpload>,
    ) -> common::Result<()> {
        TimelineStatRepo::record_uploaded_photo(&state, event.photo.created_at).await?;
        Ok(())
    }
}
