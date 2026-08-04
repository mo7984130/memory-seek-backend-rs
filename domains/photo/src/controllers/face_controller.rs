use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{get, post},
};
use common::{
    Result, ext::ResultRExt, extractors::ValidatedPath, r::R, traits::controller::ControllerRouter,
};
use types::{
    auth::user::UserId,
    photo::{FaceView, face::FaceId, person::PersonId, photo::PhotoId},
};

use crate::{PhotoState, services::face_service::FaceService};

pub struct FaceController;

impl ControllerRouter for FaceController {
    type State = PhotoState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route("/admin/full", get(Self::full_compute))
            .route("/admin/incremental", get(Self::incremental_compute))
            .route("/photo/{photo_id}", get(Self::get_faces_by_photo_id))
            .route(
                "/feature/{feature_id}/belonging/{person_id}",
                post(Self::change_belonging),
            )
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
        FaceService::compute(state, user_id, true).await.to_r_ok()
    }

    async fn incremental_compute(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<()>> {
        FaceService::compute(state, user_id, false).await.to_r_ok()
    }
}

// 修改
impl FaceController {
    /// 修改人脸归属
    async fn change_belonging(
        State(state): State<Arc<PhotoState>>,
        ValidatedPath((face_id, person_id)): ValidatedPath<(FaceId, PersonId)>,
    ) -> Result<R<()>> {
        FaceService::change_face_belonging(&state, face_id, person_id)
            .await
            .to_r_ok()
    }
}

// 查询
impl FaceController {
    async fn get_faces_by_photo_id(
        State(state): State<Arc<PhotoState>>,
        ValidatedPath(photo_id): ValidatedPath<PhotoId>,
    ) -> Result<R<Vec<FaceView>>> {
        FaceService::get_faces_by_photo_id(&state, photo_id)
            .await
            .to_r_ok()
    }
}

// 删除
impl FaceController {}
