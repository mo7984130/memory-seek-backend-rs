use std::sync::Arc;

use common_core::{BoxFuture, Result, error::AppError};
use types_admin::job::AdminJobKind;
use types_admin::job_entity::Model;
use types_core::{AdminId, UserId};

use crate::AdminState;

/// 把任务类型分发到具体业务域执行器。
pub struct Dispatcher;

impl Dispatcher {
    /// 执行一个已领取的任务，返回写入 `result` 的 JSON。
    pub async fn execute(
        state: Arc<AdminState>,
        job: &Model,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<serde_json::Value> {
        let admin = AdminId::new(UserId(job.requested_by))?;

        // 每个分支都装箱：若在 match 里内联 await 整条业务链，rustc 计算本函数
        // 状态机布局时会沿链递归展开，深度随 handler 层级累加并可能超过编译器
        // 的查询上限（`queries overflow the depth limit`）。装箱后本函数只持有
        // 指针，布局深度与 handler 内部层级无关，也不依赖 `#![recursion_limit]`。
        let job_future: BoxFuture<'_, Result<serde_json::Value>> = match job.kind {
            #[cfg(feature = "face")]
            AdminJobKind::FaceFull => {
                let visual = state.visual.clone();
                Box::pin(async move {
                    visual::admin::VisualAdminService::run_face_compute(
                        visual,
                        admin,
                        true,
                        cancellation,
                    )
                    .await
                    .map(|_| serde_json::json!({}))
                })
            }
            #[cfg(not(feature = "face"))]
            AdminJobKind::FaceFull => Box::pin(async { Err(AppError::ServiceUnavailable) }),
            #[cfg(feature = "face")]
            AdminJobKind::FaceIncremental => {
                let visual = state.visual.clone();
                Box::pin(async move {
                    visual::admin::VisualAdminService::run_face_compute(
                        visual,
                        admin,
                        false,
                        cancellation,
                    )
                    .await
                    .map(|_| serde_json::json!({}))
                })
            }
            #[cfg(not(feature = "face"))]
            AdminJobKind::FaceIncremental => Box::pin(async { Err(AppError::ServiceUnavailable) }),
            #[cfg(feature = "face")]
            AdminJobKind::PersonFullScan => {
                let visual = state.visual.clone();
                Box::pin(async move {
                    visual::admin::VisualAdminService::run_person_scan(visual, admin, cancellation)
                        .await
                        .map(|_| serde_json::json!({}))
                })
            }
            #[cfg(not(feature = "face"))]
            AdminJobKind::PersonFullScan => Box::pin(async { Err(AppError::ServiceUnavailable) }),
            AdminJobKind::FaceSecondaryCluster => {
                #[cfg(feature = "face")]
                {
                    let payload: types_admin::job::AdminJobPayload =
                        serde_json::from_value(job.payload.clone())
                            .map_err(common_core::ContextualError::from)?;
                    let threshold = payload.threshold.ok_or(AppError::InternalServerError)?;
                    let visual = state.visual.clone();
                    Box::pin(async move {
                        visual::admin::VisualAdminService::run_secondary_cluster(
                            visual,
                            admin,
                            threshold,
                            cancellation,
                        )
                        .await
                        .map(|_| serde_json::json!({}))
                    })
                }
                #[cfg(not(feature = "face"))]
                {
                    let _ = (state, admin, cancellation);
                    Box::pin(async { Err(AppError::ServiceUnavailable) })
                }
            }
            AdminJobKind::BackupScheduled => {
                let backup = state.backup.clone();
                Box::pin(async move {
                    backup::BackupService::execute_scheduled_cancellable(
                        backup,
                        admin,
                        cancellation,
                    )
                    .await
                    .map(|result| {
                        serde_json::json!({
                            "exported": result.exported,
                            "failed": result.failed,
                            "cleaned": result.cleaned,
                            "durationSecs": result.duration.as_secs_f64(),
                        })
                    })
                })
            }
            AdminJobKind::BackupManual => {
                let backup = state.backup.clone();
                Box::pin(async move {
                    backup::BackupService::execute_manual_cancellable(backup, admin, cancellation)
                        .await
                        .map(|result| {
                            serde_json::json!({
                                "runId": result.run_id,
                                "exported": result.exported,
                                "failed": result.failed,
                                "durationSecs": result.duration.as_secs_f64(),
                            })
                        })
                })
            }
            AdminJobKind::BackupRestore => {
                let payload: types_admin::job::AdminJobPayload =
                    serde_json::from_value(job.payload.clone())
                        .map_err(common_core::ContextualError::from)?;
                let request = payload.restore.ok_or(AppError::InternalServerError)?;
                let backup = state.backup.clone();
                Box::pin(async move {
                    backup::BackupService::restore(backup, admin, request, cancellation)
                        .await
                        .map(|restored| serde_json::json!({ "restored": restored }))
                })
            }
        };

        job_future.await
    }
}
