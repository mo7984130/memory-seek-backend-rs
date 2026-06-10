use std::{sync::Arc, vec};

use crate::{
    models::{
        collection::{
            CollectionPhotoAddBatchParam, CollectionPhotoAddBatchResult,
            CollectionPhotoCursorPageQuery, CollectionPhotoRemoveBatchParam,
            CollectionPhotoRemoveBatchResult,
        },
        photo::PhotoVO,
    },
    services::collection_photo_service::CollectionPhotoService,
    state::PhotoState,
};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::{delete, get},
};
use common::{Result, ext::ResultRExt, models::CursorPage, r::R, traits::controller::Controller};
use entities::{
    auth::user::UserId,
    photo::{collection::CollectionId, photo::PhotoId},
};

pub struct CollectionPhotoController;

impl Controller for CollectionPhotoController {
    type State = PhotoState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route(
                "/{collection_id}/photos",
                get(Self::get_cursor_page)
                    .post(Self::add_batch)
                    .delete(Self::remove_batch),
            )
            .route("/{collection_id}/photos/{photo_id}", delete(Self::remove))
    }

    fn public_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl CollectionPhotoController {
    async fn add_batch(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<CollectionId>,
        Json(param): Json<CollectionPhotoAddBatchParam>,
    ) -> Result<R<CollectionPhotoAddBatchResult>> {
        CollectionPhotoService::add_photos(&state, user_id, collection_id, param.photo_ids)
            .await
            .to_r_ok()
    }
}

// 查询
impl CollectionPhotoController {
    async fn get_cursor_page(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<CollectionId>,
        Query(query): Query<CollectionPhotoCursorPageQuery>,
    ) -> Result<R<CursorPage<PhotoVO, String>>> {
        let CollectionPhotoCursorPageQuery { cursor, size } = query;
        let size = size.unwrap_or(32) as u64;

        CollectionPhotoService::get_photos(&state, user_id, collection_id, cursor, size)
            .await
            .to_r_ok()
    }
}

// 修改
impl CollectionPhotoController {}

// 删除
impl CollectionPhotoController {
    async fn remove(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path((collection_id, photo_id)): Path<(CollectionId, PhotoId)>,
    ) -> Result<R<()>> {
        CollectionPhotoService::remove_photos(&state, user_id, collection_id, vec![photo_id])
            .await?;
        Ok(R::ok(()))
    }

    async fn remove_batch(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<CollectionId>,
        Json(param): Json<CollectionPhotoRemoveBatchParam>,
    ) -> Result<R<CollectionPhotoRemoveBatchResult>> {
        CollectionPhotoService::remove_photos(&state, user_id, collection_id, param.photo_ids)
            .await
            .to_r_ok()
    }
}
