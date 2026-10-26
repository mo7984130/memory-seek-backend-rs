use crate::service::BackupService;
use crate::state::BackupState;
use axum::{Extension, Json, Router, extract::State, routing::post};
use common_core::Result;
use common_web::{R, controller_router::ControllerRouter};
use std::sync::Arc;
use types_backup::RestoreRequest;
use types_core::{AdminId, UserId};

pub struct BackupController;

impl ControllerRouter for BackupController {
    type State = BackupState;

    /// 返回空的路由集合，本控制器不提供公开路由
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }

    /// 返回备份相关的管理员路由
    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route("/admin/backup/trigger", post(Self::trigger))
            .route("/admin/backup/manual", post(Self::manual))
            .route("/admin/backup/restore", post(Self::restore))
    }
}

impl BackupController {
    /// 触发一次异步的定时备份流程；仅管理员可调用
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

    /// 触发一次管理员手动备份流程；仅管理员可调用
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

    /// 按请求的备份源与恢复层级执行数据恢复；仅管理员可调用
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
