use std::sync::Arc;

#[cfg(feature = "face")]
use axum::routing::delete;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use common_core::{Result, error::AppError, types::CursorPage};
use common_web::{
    R,
    controller_router::ControllerRouter,
    ext::ToROkExt,
    extractors::{ValidatedJson, ValidatedQuery},
};
use types_admin::{
    dashboard::VisualDashboardStats,
    job::{AbandonAdminJobRequest, AdminJobQuery, AdminJobView, StartAdminJobRequest},
    user::AdminUserQuery,
};
use types_audit::{AuditQuery, AuditStatsQuery, AuditTopQuery};
use types_backup::RestoreRequest;
use types_core::{AdminId, UserId, cursor::TimeIdCursor};
use types_identity::user::UserInfo;
#[cfg(feature = "face")]
use types_visual::{MergePersonParam, PersonView, person::PersonId};

use crate::{AdminState, tasks::AdminJobService};

pub struct Controller;

impl ControllerRouter for Controller {
    type State = AdminState;

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }

    fn protected_routes() -> Router<Arc<Self::State>> {
        let router = Router::new().nest(
            "/admin",
            Router::new()
                .route("/dashboard/visuals", get(Self::visual_dashboard))
                .route("/users", get(Self::users))
                .route("/jobs", get(Self::jobs).post(Self::start_job))
                .route("/jobs/{job_id}", get(Self::get_job))
                .route("/jobs/{job_id}/cancel", post(Self::cancel_job))
                .route("/jobs/{job_id}/abandon", post(Self::abandon_job))
                .route("/audits/stats", get(Self::audit_stats))
                .route("/audits/top", get(Self::audit_top))
                .route("/audits/audit", get(Self::audits))
                .route("/backup/trigger", post(Self::backup_trigger))
                .route("/backup/manual", post(Self::backup_manual))
                .route("/backup/restore", post(Self::backup_restore)),
        );
        Self::face_routes(router)
    }
}

impl Controller {
    fn face_routes(router: Router<Arc<AdminState>>) -> Router<Arc<AdminState>> {
        #[cfg(feature = "face")]
        {
            router
                .route("/admin/visual/person/merge", post(Self::merge_person))
                .route(
                    "/admin/visual/person/{person_id}",
                    delete(Self::delete_person),
                )
        }
        #[cfg(not(feature = "face"))]
        {
            router
        }
    }

    async fn visual_dashboard(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<VisualDashboardStats>> {
        let admin = AdminId::new(user_id)?;
        visual::admin::VisualAdminService::dashboard_stats(&state.visual, admin)
            .await
            .to_r_ok()
    }

    async fn users(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(query): ValidatedQuery<AdminUserQuery>,
    ) -> Result<R<CursorPage<UserInfo, UserId>>> {
        let admin = AdminId::new(user_id)?;
        user::UserAdminService::query_users(&state.user, admin, query)
            .await
            .to_r_ok()
    }

    async fn jobs(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(query): ValidatedQuery<AdminJobQuery>,
    ) -> Result<R<CursorPage<AdminJobView, types_core::cursor::TimeIdCursor<String>>>> {
        let admin = AdminId::new(user_id)?;
        AdminJobService::list(&state, admin, query).await.to_r_ok()
    }

    async fn start_job(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(request): ValidatedJson<StartAdminJobRequest>,
    ) -> Result<R<AdminJobView>> {
        let admin = AdminId::new(user_id)?;
        AdminJobService::start(state, admin, request)
            .await
            .to_r_ok()
    }

    async fn get_job(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        Path(job_id): Path<String>,
    ) -> Result<R<AdminJobView>> {
        let admin = AdminId::new(user_id)?;
        AdminJobService::get(&state, admin, &job_id).await.to_r_ok()
    }

    async fn cancel_job(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        Path(job_id): Path<String>,
    ) -> Result<R<AdminJobView>> {
        let admin = AdminId::new(user_id)?;
        AdminJobService::cancel(&state, admin, &job_id)
            .await
            .to_r_ok()
    }

    async fn abandon_job(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        Path(job_id): Path<String>,
        ValidatedJson(request): ValidatedJson<AbandonAdminJobRequest>,
    ) -> Result<R<AdminJobView>> {
        let admin = AdminId::new(user_id)?;
        AdminJobService::abandon(&state, admin, &job_id, request)
            .await
            .to_r_ok()
    }

    async fn audit_stats(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(query): ValidatedQuery<AuditStatsQuery>,
    ) -> Result<R<Vec<types_audit::AuditStatsItem>>> {
        let admin = AdminId::new(user_id)?;
        audit::AuditQueryer::query_stats(&state.db, admin, &query)
            .await
            .to_r_ok()
    }

    async fn audit_top(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(query): ValidatedQuery<AuditTopQuery>,
    ) -> Result<R<Vec<types_audit::AuditTopItem>>> {
        let admin = AdminId::new(user_id)?;
        audit::AuditQueryer::query_top(&state.db, admin, &query)
            .await
            .to_r_ok()
    }

    async fn audits(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(query): ValidatedQuery<AuditQuery>,
    ) -> Result<R<CursorPage<types_audit::AuditItem, TimeIdCursor<types_audit::AuditId>>>> {
        let admin = AdminId::new(user_id)?;
        audit::AuditQueryer::query_events(&state.db, admin, &query)
            .await
            .to_r_ok()
    }

    async fn backup_trigger(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<AdminJobView>> {
        Self::start_backup(state, user_id, StartAdminJobRequest::BackupScheduled).await
    }

    async fn backup_manual(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<AdminJobView>> {
        Self::start_backup(state, user_id, StartAdminJobRequest::BackupManual).await
    }

    async fn start_backup(
        state: Arc<AdminState>,
        user_id: UserId,
        request: StartAdminJobRequest,
    ) -> Result<R<AdminJobView>> {
        let admin = AdminId::new(user_id)?;
        AdminJobService::start(state, admin, request)
            .await
            .to_r_ok()
    }

    async fn backup_restore(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        Json(request): Json<RestoreRequest>,
    ) -> Result<R<AdminJobView>> {
        if request.run_id != request.confirm_run_id {
            return Err(AppError::bad_request("恢复确认 ID 不匹配"));
        }

        let admin = AdminId::new(user_id)?;
        let job =
            AdminJobService::start(state, admin, StartAdminJobRequest::BackupRestore(request))
                .await?;
        Ok(R::ok(job))
    }

    #[cfg(feature = "face")]
    async fn merge_person(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(request): ValidatedJson<MergePersonParam>,
    ) -> Result<R<PersonView>> {
        let admin = AdminId::new(user_id)?;
        visual::admin::VisualAdminService::merge_person(&state.visual, admin, request)
            .await
            .to_r_ok()
    }

    #[cfg(feature = "face")]
    async fn delete_person(
        State(state): State<Arc<AdminState>>,
        Extension(user_id): Extension<UserId>,
        Path(person_id): Path<PersonId>,
    ) -> Result<R<()>> {
        let admin = AdminId::new(user_id)?;
        visual::admin::VisualAdminService::delete_person(&state.visual, admin, person_id)
            .await
            .to_r_ok()
    }
}
