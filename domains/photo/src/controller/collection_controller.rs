use axum::extract::{Path, Query, State};
use axum::routing::{get, patch, post};
use axum::Extension;
use axum::Json;
use axum::Router;
use common::error::AppError;
use common::r::R;
use std::sync::Arc;

use crate::middlewares::auth::UserId;
use crate::state::AppState;
use crate::models::collection::{
    BatchOperationResultVO, BatchPhotosDTO, CollectionCreateDTO, CollectionEditDTO,
    CollectionPhotoQuery, CollectionPhotoVO, CollectionVO,
};
use crate::models::photo::CursorPageVO;
use crate::services::collection_service::CollectionService;

pub struct CollectionController;

impl CollectionController {
    pub fn routes() -> Router<Arc<AppState>> {
        Router::new()
            .route("/", get(Self::get_list).post(Self::create))
            .route("/{id}", patch(Self::edit).delete(Self::delete))
            .route("/{id}/photos", get(Self::get_photos))
            .route("/{collection_id}/photos/{photo_id}", post(Self::add_photo).delete(Self::remove_photo))
            .route("/{collection_id}/photos/batch", post(Self::batch_add_photos).delete(Self::batch_remove_photos))
            .route("/photo/{photo_id}", get(Self::get_by_photo_id))
    }

    async fn get_list(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<Vec<CollectionVO>>, AppError> {
        let result = CollectionService::get_collection_list(
            &state.db,
            &state.redis,
            user_id.0,
            &state.encryption_key,
        )
        .await?;
        Ok(R::ok(result))
    }

    async fn create(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Json(dto): Json<CollectionCreateDTO>,
    ) -> Result<R<CollectionVO>, AppError> {
        let result =
            CollectionService::create_collection(&state.db, user_id.0, dto.name, dto.description)
                .await?;
        Ok(R::ok(result))
    }

    async fn edit(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Json(dto): Json<CollectionEditDTO>,
    ) -> Result<R<CollectionVO>, AppError> {
        let collection_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result = CollectionService::edit_collection(
            &state.db,
            user_id.0,
            collection_id,
            dto.name,
            dto.description,
        )
        .await?;
        Ok(R::ok(result))
    }

    async fn delete(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
    ) -> Result<R<()>, AppError> {
        let collection_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        CollectionService::delete_collection(&state.db, user_id.0, collection_id).await?;
        Ok(R::ok(()))
    }

    async fn get_photos(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Query(query): Query<CollectionPhotoQuery>,
    ) -> Result<R<CursorPageVO<CollectionPhotoVO, String>>, AppError> {
        let collection_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result = CollectionService::get_collection_photos(
            &state.db,
            &state.redis,
            user_id.0,
            collection_id,
            query.cursor,
            query.size.unwrap_or(20),
            &state.encryption_key,
        )
        .await?;
        Ok(R::ok(result))
    }

    async fn add_photo(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path((collection_id, photo_id)): Path<(String, String)>,
    ) -> Result<R<()>, AppError> {
        let collection_id: i64 = collection_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的收藏夹ID"))?;
        let photo_id: i64 = photo_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的照片ID"))?;
        CollectionService::add_photo_to_collection(&state.db, user_id.0, collection_id, photo_id)
            .await?;
        Ok(R::ok(()))
    }

    async fn remove_photo(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path((collection_id, photo_id)): Path<(String, String)>,
    ) -> Result<R<()>, AppError> {
        let collection_id: i64 = collection_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的收藏夹ID"))?;
        let photo_id: i64 = photo_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的照片ID"))?;
        CollectionService::remove_photo_from_collection(
            &state.db,
            user_id.0,
            collection_id,
            photo_id,
        )
        .await?;
        Ok(R::ok(()))
    }

    async fn batch_add_photos(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<String>,
        Json(dto): Json<BatchPhotosDTO>,
    ) -> Result<R<BatchOperationResultVO>, AppError> {
        let collection_id: i64 = collection_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的收藏夹ID"))?;
        let photo_ids: Vec<i64> = dto
            .photo_ids
            .into_iter()
            .filter_map(|id| id.parse().ok())
            .collect();
        let result = CollectionService::batch_add_photos_to_collection(
            &state.db,
            user_id.0,
            collection_id,
            photo_ids,
        )
        .await?;
        Ok(R::ok(result))
    }

    async fn batch_remove_photos(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<String>,
        Json(dto): Json<BatchPhotosDTO>,
    ) -> Result<R<BatchOperationResultVO>, AppError> {
        let collection_id: i64 = collection_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的收藏夹ID"))?;
        let photo_ids: Vec<i64> = dto
            .photo_ids
            .into_iter()
            .filter_map(|id| id.parse().ok())
            .collect();
        let result = CollectionService::batch_remove_photos_from_collection(
            &state.db,
            user_id.0,
            collection_id,
            photo_ids,
        )
        .await?;
        Ok(R::ok(result))
    }

    async fn get_by_photo_id(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path(photo_id): Path<String>,
    ) -> Result<R<Vec<String>>, AppError> {
        let photo_id: i64 = photo_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的照片ID"))?;
        let result =
            CollectionService::find_collection_ids_by_photo(&state.db, user_id.0, photo_id)
                .await?;
        Ok(R::ok(result))
    }
}
