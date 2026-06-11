use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{get, patch},
};
use common::{Result, ext::ResultRExt, r::R, traits::controller::ControllerRouter};
use entities::{auth::user::UserId, photo::collection::CollectionId};

use crate::{
    models::collection::{CollectionCreateParma, CollectionUpdateParam, CollectionVO},
    services::collection_service::CollectionService,
    state::PhotoState,
};

pub struct CollectionController;

impl ControllerRouter for CollectionController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route("/", get(Self::get_list).post(Self::create))
            .route("/{id}", patch(Self::update_info).delete(Self::delete))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl CollectionController {
    async fn create(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Json(data): Json<CollectionCreateParma>,
    ) -> Result<R<CollectionVO>> {
        CollectionService::create_collection(&state, user_id, data.name, data.description, false)
            .await
            .to_r_ok()
    }
}

// 查询
impl CollectionController {
    async fn get_list(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<Vec<CollectionVO>>> {
        CollectionService::get_collection_list(&state, user_id)
            .await
            .to_r_ok()
    }
}

// 修改
impl CollectionController {
    async fn update_info(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<CollectionId>,
        Json(param): Json<CollectionUpdateParam>,
    ) -> Result<R<()>> {
        CollectionService::update_collection_info(
            &state,
            user_id,
            collection_id,
            param.name,
            param.description,
        )
        .await
        .to_r_ok()
    }
}

// 删除
impl CollectionController {
    async fn delete(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<CollectionId>,
    ) -> Result<R<()>> {
        CollectionService::delete_collection(&state, user_id, collection_id)
            .await
            .to_r_ok()
    }
}
