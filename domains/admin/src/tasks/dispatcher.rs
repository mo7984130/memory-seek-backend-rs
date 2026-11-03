use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use common_core::{Result, error::AppError};
use types_admin::job::AdminJobKind;
use types_admin::job_entity::Model;
use types_core::{AdminId, UserId};

use crate::AdminState;

/// 单个任务分支的 future 类型。
///
/// 分支会被显式装箱(`Box::pin`)，目的是切断 rustc 对 coroutine 布局的递归：
/// 若 `execute` 的每个 match 分支都内联 `await` 一整条业务链，编译器计算
/// 该 async fn 状态机布局时会沿所有分支递归展开子 future 的布局，深度随
/// handler 的调用层级累加。rustc 1.99 起，`admin` 打开 `face` feature 后
/// 这个深度会超过编译器默认的查询深度上限，报
/// `queries overflow the depth limit`（指向本函数）。
///
/// 装箱后外层 coroutine 每个分支只保存一个指针，布局深度与 handler 内部层级
/// 无关；不依赖 `#![recursion_limit]` 抬高编译器的递归上限。
type JobFuture<'a> = Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send + 'a>>;

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

        let job_future: JobFuture<'_> = match job.kind {
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
