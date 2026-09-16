use std::sync::Arc;

use crate::{repo::TimelineStatRepo, services::media_service::AfterMediaUpload, state::MediaState};
use common::{Result, ext::ToOk};
use types::media::dto::timeline_stat::MonthStat;

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    /// 获取时间线统计.
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn get_monthly_stats(state: &MediaState) -> Result<Vec<MonthStat>> {
        TimelineStatRepo::get_monthly_stats(state).await?.to_ok()
    }
}

// 媒体删除时
#[step_derive::declare_transaction_step(
    ctx = crate::services::media_service::MediaDeleteContext,
    slice = crate::services::media_service::MEDIA_DELETE_STEPS,
    name = "timeline_stat_cleanup",
    owns = ["TimelineStatMapper"],
    method = on_media_delete,
)]
impl TimelineStatService {
    /// 删除媒体后扣减对应月份的时间线统计.
    async fn on_media_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::media_service::MediaDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let created_ats = ctx.medias.iter().map(|p| &p.created_at).collect::<Vec<_>>();
        crate::repo::TimelineStatRepo::decrement_by_created_ats(txn, &created_ats).await?;
        Ok(())
    }
}

// 在媒体上传之后
// 添加时间线统计
#[step_derive::declare_event_consumer(
    state = crate::state::MediaState,
    event = crate::services::media_service::AfterMediaUpload,
    slice = crate::services::media_service::AFTER_MEDIA_UPLOAD_CONSUMERS,
    name = "timeline_stat_create",
)]
impl TimelineStatService {
    #[tracing::instrument(name = "upload_media", skip_all)]
    async fn on_after_media_upload(
        &self,
        state: Arc<MediaState>,
        event: Arc<AfterMediaUpload>,
    ) -> common::Result<()> {
        TimelineStatRepo::record_uploaded_media(&state, event.media.created_at).await?;
        Ok(())
    }
}
