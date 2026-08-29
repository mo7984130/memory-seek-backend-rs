use crate::service::BackupService;
use crate::state::BackupState;
use axum::{Extension, Json, Router, extract::State, routing::post};
use common::{
    Result,
    axum::{R, controller_router::ControllerRouter},
};
use std::sync::Arc;
use types::{
    auth::user::{AdminId, UserId},
    backup::RestoreRequest,
};

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
            .route("/admin/backup/restore", post(Self::restore))
    }
}

impl BackupController {
    /// 触发一次异步的定时备份流程.
    async fn trigger(
        State(state): State<Arc<BackupState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<serde_json::Value>> {
        AdminId::new(user_id)?;

        let result = BackupService::execute_scheduled(state).await?;

        Ok(R::ok(serde_json::json!({
            "exported": result.exported,
            "failed": result.failed,
            "cleaned": result.cleaned,
            "durationSecs": result.duration.as_secs_f64(),
        })))
    }

    /// 触发一次管理员手动备份流程.
    async fn manual(
        State(state): State<Arc<BackupState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<serde_json::Value>> {
        let admin = AdminId::new(user_id)?;

        let result = BackupService::execute_manual(state, admin).await?;

        Ok(R::ok(serde_json::json!({
            "exported": result.exported,
            "failed": result.failed,
            "durationSecs": result.duration.as_secs_f64(),
        })))
    }

    async fn restore(
        State(state): State<Arc<BackupState>>,
        Extension(user_id): Extension<UserId>,
        Json(req): Json<RestoreRequest>,
    ) -> Result<R<serde_json::Value>> {
        let admin = AdminId::new(user_id)?;
        let restored = BackupService::restore(
            state,
            admin,
            req.source,
            req.tier,
            req.run_id,
            req.confirm_run_id,
        )
        .await?;
        Ok(R::ok(serde_json::json!({ "restored": restored })))
    }
}
