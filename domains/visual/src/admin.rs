use common_core::{
    Result,
    error::{
        AppError,
        contextual::ext::{IntoContextualExt, ResultExt},
    },
};
use sea_orm::{ColumnTrait, EntityTrait, QuerySelect, sea_query::Alias, sea_query::ExprTrait};
use types_admin::dashboard::VisualDashboardStats;
use types_admin::job::AdminJobKind;
use types_core::AdminId;
use types_visual::visual::{Column, Entity};
#[cfg(feature = "face")]
use types_visual::{MergePersonParam, PersonView, person::PersonId};

use crate::{repo::TimelineStatRepo, state::VisualState};

/// 跨 crate 暴露的异步入口统一返回装箱 future。
///
/// 这些入口内部是"多层 await"的长调用链(人脸计算/人物扫描/二次聚类)。
/// 若由调用方(admin 的 `Dispatcher::execute`)直接内联 await, rustc 计算外层
/// coroutine 布局时会递归进整条链; rustc 1.99 起会超过编译器的查询深度上限,
/// 报 `queries overflow the depth limit`。装箱后调用方只持有一个指针, 布局深度
/// 不再随本域内部层级增长。
#[cfg(feature = "face")]
type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// 影像域的管理员用例。
pub struct VisualAdminService;

impl VisualAdminService {
    /// 查询全站图片/视频数量、原始文件总字节数及月度时间线。
    pub async fn dashboard_stats(
        state: &VisualState,
        _admin: AdminId,
    ) -> Result<VisualDashboardStats> {
        let (total_count, total_bytes): (i64, i64) = Entity::find()
            .select_only()
            .column_as(Column::Id.count(), "total_count")
            // PostgreSQL 的 SUM(bigint) 返回 NUMERIC，显式转回 BIGINT 才能解码为 i64。
            .column_as(
                Column::Size.sum().if_null(0).cast_as(Alias::new("bigint")),
                "total_bytes",
            )
            .into_tuple()
            .one(&state.db)
            .await
            .into_contextual()?
            .unwrap_or_default();
        let monthly_timeline = TimelineStatRepo::get_monthly_stats(state).await?;

        let total_count = u64::try_from(total_count).context_error(
            "dashboard_total_count_negative",
            "影像总数为负值",
            AppError::InternalServerError,
        )?;
        let total_bytes = u64::try_from(total_bytes).context_error(
            "dashboard_total_bytes_negative",
            "影像总字节数为负值",
            AppError::InternalServerError,
        )?;

        Ok(VisualDashboardStats {
            total_count,
            total_bytes,
            monthly_timeline,
        })
    }

    /// 执行人脸计算任务。
    #[cfg(feature = "face")]
    pub fn run_face_compute(
        state: std::sync::Arc<VisualState>,
        admin: AdminId,
        full: bool,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> BoxFuture<'static, Result<()>> {
        Box::pin(async move {
            crate::services::face_service::FaceService::run_for_admin(
                state,
                admin,
                full,
                cancellation,
            )
            .await
        })
    }

    /// 执行人物全量扫描。
    #[cfg(feature = "face")]
    pub fn run_person_scan(
        state: std::sync::Arc<VisualState>,
        admin: AdminId,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> BoxFuture<'static, Result<()>> {
        Box::pin(async move {
            crate::services::person_service::PersonService::run_full_scan(
                state,
                admin,
                cancellation,
            )
            .await
        })
    }

    /// 执行未分配人脸二次聚类。
    #[cfg(feature = "face")]
    pub fn run_secondary_cluster(
        state: std::sync::Arc<VisualState>,
        admin: AdminId,
        threshold: f32,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> BoxFuture<'static, Result<()>> {
        Box::pin(async move {
            let req = types_visual::dto::person::SecondaryClusterParam { threshold };
            crate::services::person_service::PersonService::run_secondary_cluster(
                state,
                admin,
                req,
                cancellation,
            )
            .await
        })
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
