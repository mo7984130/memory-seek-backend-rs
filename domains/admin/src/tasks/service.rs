use std::sync::Arc;

use audit::{AuditEvent, AuditRecorder};
use common_core::{
    Result,
    error::{AppError, ContextualError},
    types::CursorPage,
};
use types_admin::job::{
    AbandonAdminJobRequest, AdminJobKind, AdminJobQuery, AdminJobView, StartAdminJobRequest,
};
use types_admin::job_entity::Model;
use types_core::{AdminId, UserId, cursor::TimeIdCursor};

use crate::AdminState;
use crate::tasks::repo;

/// 后台任务用例入口。
///
/// 只负责参数校验、事务边界、审计与视图转换；领取与执行见 `supervisor` / `runner`。
pub struct AdminJobService;

impl AdminJobService {
    /// 创建排队任务。
    ///
    /// 并发创建同类型任务时由数据库唯一索引兜底，冲突映射为 409。
    pub async fn start(
        state: Arc<AdminState>,
        admin: AdminId,
        request: StartAdminJobRequest,
    ) -> Result<AdminJobView> {
        let payload = payload_for(&request)?;
        let kind = request.kind;
        let actor = admin.into_inner().0;

        let row = common_db::db_transaction!(&state.db, |txn| {
            let row = match repo::insert_queued(txn, kind, actor, payload.clone()).await {
                Ok(row) => row,
                Err(error) if repo::is_active_conflict(&error) => {
                    return Err(conflict("同类型后台任务已在运行"));
                }
                Err(error) => return Err(ContextualError::from(error)),
            };

            AuditRecorder::append(
                txn,
                AuditEvent::new("admin.job_started")
                    .with_actor(actor)
                    .with_detail(serde_json::json!({
                        "jobId": row.id,
                        "kind": kind.as_str(),
                    })),
            )
            .await?;
            Ok(row)
        })
        .await?;

        state.job_wake.notify_one();
        Ok(Self::to_view(row))
    }

    /// 分页查询任务。
    pub async fn list(
        state: &AdminState,
        _admin: AdminId,
        query: AdminJobQuery,
    ) -> Result<CursorPage<AdminJobView, TimeIdCursor<String>>> {
        let page = repo::list(&state.db, &query).await?;
        Ok(page.map_records(|rows| rows.into_iter().map(Self::to_view).collect()))
    }

    /// 查询单个任务。
    pub async fn get(state: &AdminState, _admin: AdminId, id: &str) -> Result<AdminJobView> {
        let row = repo::find_by_id(&state.db, id)
            .await?
            .ok_or_else(|| AppError::not_found("后台任务不存在"))?;
        Ok(Self::to_view(row))
    }

    /// 取消任务。
    ///
    /// 排队中的任务直接取消；运行中的任务写入取消标记，由执行实例在下一个心跳周期确认。
    pub async fn cancel(state: &AdminState, admin: AdminId, id: &str) -> Result<AdminJobView> {
        let actor = admin.into_inner().0;

        common_db::db_transaction!(&state.db, |txn| {
            let row = repo::find_by_id(txn, id).await?.ok_or_else(|| {
                ContextualError::warn_without_source(
                    "admin_job_not_found",
                    "后台任务不存在",
                    AppError::not_found("后台任务不存在"),
                )
            })?;

            match row.status {
                types_admin::job::AdminJobStatus::Queued => {
                    if !repo::cancel_queued(txn, id).await? {
                        return Err(conflict("任务状态已改变，请重试取消"));
                    }
                }
                types_admin::job::AdminJobStatus::Running => {
                    if row.kind == AdminJobKind::FaceFull {
                        return Err(conflict("全量人脸重算已清空旧数据，运行期间不能安全取消"));
                    }
                    if !repo::request_cancel(txn, id).await? {
                        return Err(conflict("任务状态已改变，请重试取消"));
                    }
                }
                types_admin::job::AdminJobStatus::CancelRequested => {}
                _ => return Err(conflict("任务已结束，不能取消")),
            }

            AuditRecorder::append(
                txn,
                AuditEvent::new("admin.job_cancel_requested")
                    .with_actor(actor)
                    .with_detail(serde_json::json!({ "jobId": id })),
            )
            .await?;
            Ok(())
        })
        .await?;

        Self::get(state, admin, id).await
    }

    /// 人工放弃一个运行中的任务。
    ///
    /// 用于“实例仍在但任务卡死”的场景；实例失联由 supervisor 自动回收。
    pub async fn abandon(
        state: &AdminState,
        admin: AdminId,
        id: &str,
        request: AbandonAdminJobRequest,
    ) -> Result<AdminJobView> {
        let actor = admin.into_inner().0;

        common_db::db_transaction!(&state.db, |txn| {
            let row = repo::find_by_id(txn, id).await?.ok_or_else(|| {
                ContextualError::warn_without_source(
                    "admin_job_not_found",
                    "后台任务不存在",
                    AppError::not_found("后台任务不存在"),
                )
            })?;

            if !matches!(
                row.status,
                types_admin::job::AdminJobStatus::Running
                    | types_admin::job::AdminJobStatus::CancelRequested
            ) {
                return Err(conflict("任务已结束，不能放弃"));
            }

            if !repo::abandon(txn, id, request.reason.clone()).await? {
                return Err(conflict("任务状态已改变，请重试"));
            }

            AuditRecorder::append(
                txn,
                AuditEvent::new("admin.job_abandoned")
                    .with_actor(actor)
                    .with_detail(serde_json::json!({
                        "jobId": id,
                        "reason": request.reason.clone(),
                    })),
            )
            .await?;
            Ok(())
        })
        .await?;

        Self::get(state, admin, id).await
    }

    /// 把持久化记录转换为接口视图。
    fn to_view(row: Model) -> AdminJobView {
        AdminJobView {
            id: row.id,
            kind: row.kind,
            status: row.status,
            requested_by: UserId(row.requested_by),
            attempts: row.attempts.max(0) as u32,
            worker_id: row.worker_id,
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
            result: row.result,
            error: row.error,
        }
    }
}

/// 构造并校验任务执行参数。
fn payload_for(request: &StartAdminJobRequest) -> Result<serde_json::Value> {
    match (request.kind, request.threshold) {
        (AdminJobKind::FaceSecondaryCluster, Some(threshold)) => {
            Ok(serde_json::json!({ "threshold": threshold }))
        }
        (AdminJobKind::FaceSecondaryCluster, None) => {
            Err(AppError::bad_request("二次聚类任务需要提供 threshold"))
        }
        (_, Some(_)) => Err(AppError::bad_request("仅二次聚类任务需要提供 threshold")),
        (_, None) => Ok(serde_json::Value::Object(Default::default())),
    }
}

/// 构造任务冲突错误。
fn conflict(message: &'static str) -> ContextualError<AppError> {
    ContextualError::warn_without_source("admin_job_conflict", message, AppError::conflict(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_cluster_requires_threshold() {
        let request = StartAdminJobRequest {
            kind: AdminJobKind::FaceSecondaryCluster,
            threshold: None,
        };
        assert!(payload_for(&request).is_err());
    }

    #[test]
    fn other_jobs_reject_threshold() {
        let request = StartAdminJobRequest {
            kind: AdminJobKind::BackupManual,
            threshold: Some(0.6),
        };
        assert!(payload_for(&request).is_err());
    }

    #[test]
    fn threshold_is_persisted() {
        let request = StartAdminJobRequest {
            kind: AdminJobKind::FaceSecondaryCluster,
            threshold: Some(0.75),
        };
        let payload = payload_for(&request).unwrap();
        assert_eq!(payload, serde_json::json!({ "threshold": 0.75 }));
    }
}
