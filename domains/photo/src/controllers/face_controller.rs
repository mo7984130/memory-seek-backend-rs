use std::sync::Arc;

use axum::{Extension, Router, extract::State, routing::get};
use common::{Result, ext::ResultRExt, r::R, traits::controller::ControllerRouter};
use entities::auth::user::UserId;

use crate::{PhotoState, services::face_service::FaceService};

pub struct FaceController;

impl ControllerRouter for FaceController {
    type State = PhotoState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route("/admin/full", get(Self::full_compute))
            .route("/admin/incremental", get(Self::incremental_compute))
    }

    fn public_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl FaceController {
    async fn full_compute(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<()>> {
        FaceService::full_compute(&state, user_id).await.to_r_ok()
    }
}

// 修改
impl FaceController {
    async fn incremental_compute(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<()>> {
        FaceService::incremental_compute(&state, user_id).await.to_r_ok()
    }
}

// 查询
impl FaceController {}

// 删除
impl FaceController {}
