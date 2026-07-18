use crate::runner::BackupRunner;
use crate::state::BackupState;
use axum::{Extension, Router, extract::State, routing::post};
use common::{
    Result, error::AppError, ext::ResultErrExt, r::R, traits::controller::ControllerRouter,
};
use entities::auth::user::UserId;
use std::sync::Arc;

pub struct BackupController;

impl ControllerRouter for BackupController {
    type State = BackupState;

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route("/admin/backup/trigger", post(Self::trigger))
            .route("/admin/backup/manual", post(Self::manual))
    }
}

impl BackupController {
    async fn trigger(
        State(state): State<Arc<BackupState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<serde_json::Value>> {
        if user_id.0 != 1 {
            return Err(AppError::forbidden("仅管理员可执行定时备份"));
        }

        let result = BackupRunner::execute_scheduled(state)
            .await
            .trace_internal_err("backup_exec_err", "定时备份执行失败")?;

        Ok(R::ok(serde_json::json!({
            "exported": result.exported,
            "failed": result.failed,
            "cleaned": result.cleaned,
            "durationSecs": result.duration.as_secs_f64(),
        })))
    }

    async fn manual(
        State(state): State<Arc<BackupState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<serde_json::Value>> {
        if user_id.0 != 1 {
            return Err(AppError::forbidden("仅管理员可执行手动备份"));
        }

        let result = BackupRunner::execute_manual(state)
            .await
            .trace_internal_err("backup_manual_err", "手动备份执行失败")?;

        Ok(R::ok(serde_json::json!({
            "exported": result.exported,
            "failed": result.failed,
            "durationSecs": result.duration.as_secs_f64(),
        })))
    }
}
