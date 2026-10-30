use common_core::{Result, error::contextual::ext::IntoContextualExt};
use sea_orm::{ColumnTrait, EntityTrait, QuerySelect};
use types_admin::dashboard::VisualDashboardStats;
use types_admin::job::AdminJobKind;
use types_core::AdminId;
use types_visual::visual::{Column, Entity};
#[cfg(feature = "face")]
use types_visual::{MergePersonParam, PersonView, person::PersonId};

use crate::{repo::TimelineStatRepo, state::VisualState};

/// 影像域的管理员用例。
pub struct VisualAdminService;

impl VisualAdminService {
    /// 查询全站图片/视频数量、原始文件总字节数及月度时间线。
    pub async fn dashboard_stats(
        state: &VisualState,
        _admin: AdminId,
    ) -> Result<VisualDashboardStats> {
        let (total_count, total_bytes): (i64, Option<i64>) = Entity::find()
            .select_only()
            .column_as(Column::Id.count(), "total_count")
            .column_as(Column::Size.sum(), "total_bytes")
            .into_tuple()
            .one(&state.db)
            .await
            .into_contextual()?
            .unwrap_or((0, None));
        let monthly_timeline = TimelineStatRepo::get_monthly_stats(state).await?;

        Ok(VisualDashboardStats {
            total_count: total_count.max(0) as u64,
            total_bytes: total_bytes.unwrap_or_default().max(0) as u64,
            monthly_timeline,
        })
    }

    /// 执行人脸计算任务。
    #[cfg(feature = "face")]
    pub async fn run_face_compute(
        state: std::sync::Arc<VisualState>,
        admin: AdminId,
        full: bool,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        crate::services::face_service::FaceService::run_for_admin(state, admin, full, cancellation)
            .await
    }

    /// 执行人物全量扫描。
    #[cfg(feature = "face")]
    pub async fn run_person_scan(
        state: std::sync::Arc<VisualState>,
        admin: AdminId,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        crate::services::person_service::PersonService::run_full_scan(state, admin, cancellation)
            .await
    }

    /// 执行未分配人脸二次聚类。
    #[cfg(feature = "face")]
    pub async fn run_secondary_cluster(
        state: std::sync::Arc<VisualState>,
        admin: AdminId,
        threshold: f32,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let req = types_visual::dto::person::SecondaryClusterParam { threshold };
        crate::services::person_service::PersonService::run_secondary_cluster(
            state,
            admin,
            req,
            cancellation,
        )
        .await
    }

    /// 识别任务类型是否由影像域执行。
    pub const fn supports_job(kind: AdminJobKind) -> bool {
        matches!(
            kind,
            AdminJobKind::FaceFull
                | AdminJobKind::FaceIncremental
                | AdminJobKind::PersonFullScan
                | AdminJobKind::FaceSecondaryCluster
        )
    }

    /// 管理员合并人物。
    #[cfg(feature = "face")]
    pub async fn merge_person(
        state: &VisualState,
        admin: AdminId,
        request: MergePersonParam,
    ) -> Result<PersonView> {
        crate::services::person_service::PersonService::merge_person(state, admin, request).await
    }

    /// 管理员删除人物及其关联数据。
    #[cfg(feature = "face")]
    pub async fn delete_person(
        state: &VisualState,
        admin: AdminId,
        person_id: PersonId,
    ) -> Result<()> {
        crate::services::person_service::PersonService::delete_person(state, admin, person_id).await
    }
}
