use std::sync::Arc;

use crate::{
    models::{
        collection::{
            CollectionPhotoAddBatchParam, CollectionPhotoAddBatchResult,
            CollectionPhotoCursorPageParam, CollectionPhotoRemoveBatchParam,
            CollectionPhotoRemoveBatchResult, PhotoCollectionResult,
        },
        photo::PhotoResult,
    },
    services::collection_photo_service::CollectionPhotoService,
    state::PhotoState,
};
use axum::{
    Extension, Router,
    extract::State,
    routing::{delete, get},
};
use common::{
    Result,
    ext::ResultRExt,
    extractors::{ValidatedJson, ValidatedPath, ValidatedQuery},
    models::{CursorPage, TimeIdCursor},
    r::R,
    traits::controller::ControllerRouter,
};
use types::{
    auth::user::UserId,
    photo::{collection::CollectionId, models::PhotoIds, photo::PhotoId},
};

pub struct CollectionPhotoController;

impl ControllerRouter for CollectionPhotoController {
    type State = PhotoState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route("/by-photo/{photo_id}", get(Self::get_collections_by_photo))
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

// 查询照片所属收藏夹
impl CollectionPhotoController {
    async fn get_collections_by_photo(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(photo_id): ValidatedPath<PhotoId>,
    ) -> Result<R<Vec<PhotoCollectionResult>>> {
        CollectionPhotoService::get_collections_by_photo(&state, user_id, photo_id)
            .await
            .to_r_ok()
    }
}

// 创建
impl CollectionPhotoController {
    async fn add_batch(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
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
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedQuery(query): ValidatedQuery<CollectionPhotoCursorPageParam>,
    ) -> Result<R<CursorPage<PhotoResult, String>>> {
        let CollectionPhotoCursorPageParam { cursor, size } = query;

        let cursor = cursor.map(TimeIdCursor::<PhotoId>::decode).transpose()?;

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
        ValidatedPath((collection_id, photo_id)): ValidatedPath<(CollectionId, PhotoId)>,
    ) -> Result<R<()>> {
        CollectionPhotoService::remove_photos(
            &state,
            user_id,
            collection_id,
            PhotoIds::new(vec![photo_id]).unwrap(),
        )
        .await?;
        Ok(R::ok(()))
    }

    async fn remove_batch(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedJson(param): ValidatedJson<CollectionPhotoRemoveBatchParam>,
    ) -> Result<R<CollectionPhotoRemoveBatchResult>> {
        CollectionPhotoService::remove_photos(&state, user_id, collection_id, param.photo_ids)
            .await
            .to_r_ok()
    }
}
