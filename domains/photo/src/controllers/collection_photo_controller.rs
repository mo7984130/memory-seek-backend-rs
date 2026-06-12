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
    Extension, Router,
    extract::{Path, State},
    routing::{delete, get},
};
use common::{
    Result, ext::ResultRExt, extractors::{ValidatedJson, ValidatedQuery}, models::CursorPage, r::R,
    traits::controller::ControllerRouter,
};
use entities::{
    auth::user::UserId,
    photo::{collection::CollectionId, photo::PhotoId},
};

pub struct CollectionPhotoController;

impl ControllerRouter for CollectionPhotoController {
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
        ValidatedJson(param): ValidatedJson<CollectionPhotoAddBatchParam>,
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
        ValidatedQuery(query): ValidatedQuery<CollectionPhotoCursorPageQuery>,
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
        ValidatedJson(param): ValidatedJson<CollectionPhotoRemoveBatchParam>,
    ) -> Result<R<CollectionPhotoRemoveBatchResult>> {
        CollectionPhotoService::remove_photos(&state, user_id, collection_id, param.photo_ids)
            .await
            .to_r_ok()
    }
}
