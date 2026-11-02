use std::sync::Arc;

use common_core::{Result, error::AppError};
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

        match job.kind {
            #[cfg(feature = "face")]
            AdminJobKind::FaceFull => visual::admin::VisualAdminService::run_face_compute(
                state.visual.clone(),
                admin,
                true,
                cancellation,
            )
            .await
            .map(|_| serde_json::json!({})),
            #[cfg(not(feature = "face"))]
            AdminJobKind::FaceFull => Err(AppError::ServiceUnavailable),
            #[cfg(feature = "face")]
            AdminJobKind::FaceIncremental => visual::admin::VisualAdminService::run_face_compute(
                state.visual.clone(),
                admin,
                false,
                cancellation,
            )
            .await
            .map(|_| serde_json::json!({})),
            #[cfg(not(feature = "face"))]
            AdminJobKind::FaceIncremental => Err(AppError::ServiceUnavailable),
            #[cfg(feature = "face")]
            AdminJobKind::PersonFullScan => visual::admin::VisualAdminService::run_person_scan(
                state.visual.clone(),
                admin,
                cancellation,
            )
            .await
            .map(|_| serde_json::json!({})),
            #[cfg(not(feature = "face"))]
            AdminJobKind::PersonFullScan => Err(AppError::ServiceUnavailable),
            AdminJobKind::FaceSecondaryCluster => {
                #[cfg(feature = "face")]
                {
                    let payload: types_admin::job::AdminJobPayload =
                        serde_json::from_value(job.payload.clone())
                            .map_err(common_core::ContextualError::from)?;
                    let threshold = payload.threshold.ok_or(AppError::InternalServerError)?;
                    visual::admin::VisualAdminService::run_secondary_cluster(
                        state.visual.clone(),
                        admin,
                        threshold,
                        cancellation,
                    )
                    .await
                    .map(|_| serde_json::json!({}))
                }
                #[cfg(not(feature = "face"))]
                {
                    let _ = (state, admin, cancellation);
                    Err(AppError::ServiceUnavailable)
                }
            }
            AdminJobKind::BackupScheduled => backup::BackupService::execute_scheduled_cancellable(
                state.backup.clone(),
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
            }),
            AdminJobKind::BackupManual => backup::BackupService::execute_manual_cancellable(
                state.backup.clone(),
                admin,
                cancellation,
            )
            .await
            .map(|result| {
                serde_json::json!({
                    "runId": result.run_id,
                    "exported": result.exported,
                    "failed": result.failed,
                    "durationSecs": result.duration.as_secs_f64(),
                })
            }),
        }
    }
}
