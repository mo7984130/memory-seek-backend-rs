use audit::{AuditEvent, AuditRecorder};
use common_core::{
    Result,
    error::{AppError, contextual::ext::IntoContextualExt},
    time::now,
    types::CursorPage,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use serde_json::json;
use types_admin::{
    job::{AdminJobKind, AdminJobQuery, AdminJobStatus, AdminJobView, StartAdminJobRequest},
    job_entity::{ActiveModel, Column, Entity, Model},
};
use types_core::{AdminId, cursor::TimeIdCursor};
use uuid::Uuid;

use crate::AdminState;

/// 启动时把上次进程遗留的未完成任务标记为中断，并建立跨进程去重约束。
pub async fn prepare_job_store(db: &DatabaseConnection) -> Result<()> {
    Entity::update_many()
        .col_expr(
            Column::Status,
            sea_orm::sea_query::Expr::value("interrupted"),
        )
        .col_expr(
            Column::FinishedAt,
            sea_orm::sea_query::Expr::current_timestamp(),
        )
        .filter(Column::Status.is_in(["queued", "running", "cancel_requested"]))
        .exec(db)
        .await
        .into_contextual()?;

    Ok(())
}

pub struct AdminJobService;

impl AdminJobService {
    pub async fn start(
        state: std::sync::Arc<AdminState>,
        admin: AdminId,
        request: StartAdminJobRequest,
    ) -> Result<AdminJobView> {
        if matches!(request.kind, AdminJobKind::FaceSecondaryCluster) != request.threshold.is_some()
        {
            return Err(AppError::bad_request("仅二次聚类任务需要提供 threshold"));
        }

        let _guard = state.task_start_lock.lock().await;
        let existing = Entity::find()
            .filter(Column::Kind.eq(request.kind.as_str()))
            .filter(Column::Status.is_in(["queued", "running", "cancel_requested"]))
            .one(&state.db)
            .await
            .into_contextual()?;
        if existing.is_some() {
            return Err(AppError::conflict("同类型后台任务已在运行"));
        }

        let id = Uuid::new_v4().to_string();
        let row = common_db::db_transaction!(&state.db, |txn| {
            let row = ActiveModel {
                id: Set(id.clone()),
                kind: Set(request.kind.as_str().to_owned()),
                status: Set(AdminJobStatus::Queued.as_str().to_owned()),
                requested_by: Set(admin.into_inner().0),
                result: Set(None),
                error: Set(None),
                created_at: Set(now()),
                started_at: Set(None),
                finished_at: Set(None),
                cancel_requested: Set(false),
            }
            .insert(txn)
            .await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("admin.job_started")
                    .with_actor(admin.into_inner().0)
                    .with_detail(json!({ "jobId": id, "kind": request.kind.as_str() })),
            )
            .await?;
            Ok(row)
        })
        .await?;

        let token = tokio_util::sync::CancellationToken::new();
        state.task_tokens.insert(id.clone(), token.clone());
        let request_threshold = request.threshold;
        let state_for_task = state.clone();
        let id_for_task = id.clone();
        state
            .task_manager
            .spawn("admin-jobs", move |shutdown| async move {
                let run = Self::run(
                    state_for_task,
                    admin,
                    id_for_task,
                    request.kind,
                    request_threshold,
                    token.clone(),
                );
                tokio::pin!(run);
                tokio::select! {
                    biased;
                    _ = shutdown.cancelled() => token.cancel(),
                    _ = &mut run => return,
                }
                run.await;
            });

        Self::to_view(row)
    }

    pub async fn list(
        state: &AdminState,
        _admin: AdminId,
        query: AdminJobQuery,
    ) -> Result<CursorPage<AdminJobView, TimeIdCursor<String>>> {
        let mut select = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id);
        if let Some(kind) = query.kind {
            select = select.filter(Column::Kind.eq(kind.as_str()));
        }
        if let Some(status) = query.status {
            select = select.filter(Column::Status.eq(status.as_str()));
        }
        if let Some(cursor) = query.cursor {
            select = select.filter(
                Condition::any()
                    .add(Column::CreatedAt.lt(cursor.time_at))
                    .add(
                        Condition::all()
                            .add(Column::CreatedAt.eq(cursor.time_at))
                            .add(Column::Id.lt(cursor.id)),
                    ),
            );
        }
        let rows = select
            .limit(query.size + 1)
            .all(&state.db)
            .await
            .into_contextual()?;
        let page =
            CursorPage::from_oversize(rows, query.size).with_next_cursor(|row| TimeIdCursor {
                time_at: row.created_at,
                id: row.id.clone(),
            });
        Ok(CursorPage {
            records: page
                .records
                .into_iter()
                .map(Self::to_view)
                .collect::<Result<Vec<_>>>()?,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }

    pub async fn get(state: &AdminState, _admin: AdminId, id: &str) -> Result<AdminJobView> {
        let row = Entity::find_by_id(id)
            .one(&state.db)
            .await
            .into_contextual()?
            .ok_or_else(|| AppError::not_found("后台任务不存在"))?;
        Self::to_view(row)
    }

    pub async fn cancel(state: &AdminState, admin: AdminId, id: &str) -> Result<AdminJobView> {
        let row = Entity::find_by_id(id)
            .one(&state.db)
            .await
            .into_contextual()?
            .ok_or_else(|| AppError::not_found("后台任务不存在"))?;
        let status = parse_status(&row.status)?;
        match status {
            AdminJobStatus::Queued => {
                let update = Entity::update_many()
                    .col_expr(Column::Status, sea_orm::sea_query::Expr::value("cancelled"))
                    .col_expr(
                        Column::FinishedAt,
                        sea_orm::sea_query::Expr::current_timestamp(),
                    )
                    .filter(Column::Id.eq(id))
                    .filter(Column::Status.eq("queued"))
                    .exec(&state.db)
                    .await
                    .into_contextual()?;
                if update.rows_affected == 0 {
                    return Err(AppError::conflict("任务状态已改变，请重试取消"));
                }
                if let Some(token) = state.task_tokens.get(id) {
                    token.cancel();
                }
            }
            AdminJobStatus::Running => {
                if row.kind == AdminJobKind::FaceFull.as_str() {
                    return Err(AppError::conflict(
                        "全量人脸重算已清空旧数据，运行期间不能安全取消",
                    ));
                }
                let update = Entity::update_many()
                    .col_expr(
                        Column::Status,
                        sea_orm::sea_query::Expr::value("cancel_requested"),
                    )
                    .col_expr(
                        Column::CancelRequested,
                        sea_orm::sea_query::Expr::value(true),
                    )
                    .filter(Column::Id.eq(id))
                    .filter(Column::Status.eq("running"))
                    .exec(&state.db)
                    .await
                    .into_contextual()?;
                if update.rows_affected == 0 {
                    return Err(AppError::conflict("任务状态已改变，请重试取消"));
                }
                if let Some(token) = state.task_tokens.get(id) {
                    token.cancel();
                }
            }
            AdminJobStatus::CancelRequested => {}
            _ => return Err(AppError::conflict("任务已结束，不能取消")),
        }
        common_db::db_transaction!(&state.db, |txn| {
            AuditRecorder::append(
                txn,
                AuditEvent::new("admin.job_cancel_requested")
                    .with_actor(admin.into_inner().0)
                    .with_detail(json!({ "jobId": id })),
            )
            .await?;
            Ok(())
        })
        .await?;
        Self::get(state, admin, id).await
    }

    async fn run(
        state: std::sync::Arc<AdminState>,
        admin: AdminId,
        id: String,
        kind: AdminJobKind,
        _threshold: Option<f32>,
        token: tokio_util::sync::CancellationToken,
    ) {
        if token.is_cancelled() {
            Self::finish(&state, &id, AdminJobStatus::Cancelled, None, None).await;
            state.task_tokens.remove(&id);
            return;
        }

        let started = Entity::update_many()
            .col_expr(Column::Status, sea_orm::sea_query::Expr::value("running"))
            .col_expr(
                Column::StartedAt,
                sea_orm::sea_query::Expr::current_timestamp(),
            )
            .filter(Column::Id.eq(&id))
            .filter(Column::Status.eq("queued"))
            .exec(&state.db)
            .await;
        match started {
            Ok(result) if result.rows_affected > 0 => {}
            _ => {
                state.task_tokens.remove(&id);
                return;
            }
        }

        let result = match kind {
            #[cfg(feature = "face")]
            AdminJobKind::FaceFull => visual::admin::VisualAdminService::run_face_compute(
                state.visual.clone(),
                admin,
                true,
                token.clone(),
            )
            .await
            .map(|_| json!({})),
            #[cfg(not(feature = "face"))]
            AdminJobKind::FaceFull => Err(AppError::ServiceUnavailable),
            #[cfg(feature = "face")]
            AdminJobKind::FaceIncremental => visual::admin::VisualAdminService::run_face_compute(
                state.visual.clone(),
                admin,
                false,
                token.clone(),
            )
            .await
            .map(|_| json!({})),
            #[cfg(not(feature = "face"))]
            AdminJobKind::FaceIncremental => Err(AppError::ServiceUnavailable),
            #[cfg(feature = "face")]
            AdminJobKind::PersonFullScan => visual::admin::VisualAdminService::run_person_scan(
                state.visual.clone(),
                admin,
                token.clone(),
            )
            .await
            .map(|_| json!({})),
            #[cfg(not(feature = "face"))]
            AdminJobKind::PersonFullScan => Err(AppError::ServiceUnavailable),
            #[cfg(feature = "face")]
            AdminJobKind::FaceSecondaryCluster => {
                visual::admin::VisualAdminService::run_secondary_cluster(
                    state.visual.clone(),
                    admin,
                    _threshold.unwrap_or(0.7),
                    token.clone(),
                )
                .await
                .map(|_| json!({}))
            }
            #[cfg(not(feature = "face"))]
            AdminJobKind::FaceSecondaryCluster => Err(AppError::ServiceUnavailable),
            AdminJobKind::BackupScheduled => backup::BackupService::execute_scheduled_cancellable(
                state.backup.clone(),
                admin,
                token.clone(),
            )
            .await
            .map(|result| {
                json!({
                    "exported": result.exported,
                    "failed": result.failed,
                    "cleaned": result.cleaned,
                    "durationSecs": result.duration.as_secs_f64(),
                })
            }),
            AdminJobKind::BackupManual => backup::BackupService::execute_manual_cancellable(
                state.backup.clone(),
                admin,
                token.clone(),
            )
            .await
            .map(|result| {
                json!({
                    "runId": result.run_id,
                    "exported": result.exported,
                    "failed": result.failed,
                    "durationSecs": result.duration.as_secs_f64(),
                })
            }),
        };

        let (status, payload, error) = match result {
            Ok(value) => (AdminJobStatus::Succeeded, Some(value), None),
            Err(error) if token.is_cancelled() => {
                (AdminJobStatus::Cancelled, None, Some(error.to_string()))
            }
            Err(error) => (AdminJobStatus::Failed, None, Some(error.to_string())),
        };
        Self::finish(&state, &id, status, payload, error).await;
        state.task_tokens.remove(&id);
    }

    async fn finish(
        state: &AdminState,
        id: &str,
        status: AdminJobStatus,
        result: Option<serde_json::Value>,
        error: Option<String>,
    ) {
        let _ = Entity::update_many()
            .col_expr(
                Column::Status,
                sea_orm::sea_query::Expr::value(status.as_str()),
            )
            .col_expr(
                Column::FinishedAt,
                sea_orm::sea_query::Expr::current_timestamp(),
            )
            .col_expr(Column::Result, sea_orm::sea_query::Expr::value(result))
            .col_expr(Column::Error, sea_orm::sea_query::Expr::value(error))
            .filter(Column::Id.eq(id))
            .exec(&state.db)
            .await;
    }

    fn to_view(row: Model) -> Result<AdminJobView> {
        Ok(AdminJobView {
            id: row.id,
            kind: parse_kind(&row.kind)?,
            status: parse_status(&row.status)?,
            requested_by: types_core::UserId(row.requested_by),
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
            result: row.result,
            error: row.error,
        })
    }
}

fn parse_kind(value: &str) -> Result<AdminJobKind> {
    match value {
        "face_full" => Ok(AdminJobKind::FaceFull),
        "face_incremental" => Ok(AdminJobKind::FaceIncremental),
        "person_full_scan" => Ok(AdminJobKind::PersonFullScan),
        "face_secondary_cluster" => Ok(AdminJobKind::FaceSecondaryCluster),
        "backup_scheduled" => Ok(AdminJobKind::BackupScheduled),
        "backup_manual" => Ok(AdminJobKind::BackupManual),
        _ => Err(AppError::InternalServerError),
    }
}

fn parse_status(value: &str) -> Result<AdminJobStatus> {
    match value {
        "queued" => Ok(AdminJobStatus::Queued),
        "running" => Ok(AdminJobStatus::Running),
        "succeeded" => Ok(AdminJobStatus::Succeeded),
        "failed" => Ok(AdminJobStatus::Failed),
        "cancel_requested" => Ok(AdminJobStatus::CancelRequested),
        "cancelled" => Ok(AdminJobStatus::Cancelled),
        "interrupted" => Ok(AdminJobStatus::Interrupted),
        _ => Err(AppError::InternalServerError),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_job_names_round_trip() {
        for kind in [
            AdminJobKind::FaceFull,
            AdminJobKind::FaceIncremental,
            AdminJobKind::PersonFullScan,
            AdminJobKind::FaceSecondaryCluster,
            AdminJobKind::BackupScheduled,
            AdminJobKind::BackupManual,
        ] {
            assert_eq!(parse_kind(kind.as_str()).unwrap(), kind);
        }
        for status in [
            AdminJobStatus::Queued,
            AdminJobStatus::Running,
            AdminJobStatus::Succeeded,
            AdminJobStatus::Failed,
            AdminJobStatus::CancelRequested,
            AdminJobStatus::Cancelled,
            AdminJobStatus::Interrupted,
        ] {
            assert_eq!(parse_status(status.as_str()).unwrap(), status);
        }
    }

    #[test]
    fn unsupported_persisted_job_names_are_rejected() {
        assert!(parse_kind("unknown").is_err());
        assert!(parse_status("unknown").is_err());
    }
}
