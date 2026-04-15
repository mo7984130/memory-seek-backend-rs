use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::Extension;
use axum::Json;
use axum::Router;
use common::error::AppError;
use common::r::R;
use std::sync::Arc;

use crate::middlewares::auth::UserId;
use crate::state::AppState;
use crate::models::face::{
    FaceFeatureVO, FacePersonSimpleVO, FacePersonVO, MergePersonRequest, PersonPageQuery,
    PersonSearchQuery, RenamePersonRequest,
};
use crate::models::photo::{CursorPageVO, PhotoVO};
use crate::services::face_service::FaceService;
use crate::services::feature_service::FeatureService;

pub struct FaceController;

impl FaceController {
    pub fn routes() -> Router<Arc<AppState>> {
        Router::new()
            .route("/person", get(Self::get_person_page))
            .route("/person/all", get(Self::get_all_person))
            .route("/person/search", get(Self::search_person))
            .route("/person/{id}", get(Self::get_person_info))
            .route("/person/{id}/name", post(Self::rename_person))
            .route("/person/{id}/photo", get(Self::get_person_photo))
            .route("/person/merge", post(Self::merge_person))
            .route("/person/{id}", delete(Self::delete_person))
            .route("/feature/{photo_id}", get(Self::get_photo_features))
            .route("/feature/{feature_id}/belonging/{person_id}", post(Self::change_face_belonging))
    }

    async fn get_person_page(
        State(state): State<Arc<AppState>>,
        Query(query): Query<PersonPageQuery>,
    ) -> Result<R<CursorPageVO<FacePersonVO, String>>, AppError> {
        let result =
            FaceService::get_person_page(&state.db, &state.redis, query, &state.encryption_key)
                .await?;
        Ok(R::ok(result))
    }

    async fn get_all_person(
        State(state): State<Arc<AppState>>,
    ) -> Result<R<Vec<FacePersonSimpleVO>>, AppError> {
        let result = FaceService::get_all_person(&state.db).await?;
        Ok(R::ok(result))
    }

    async fn search_person(
        State(state): State<Arc<AppState>>,
        Query(query): Query<PersonSearchQuery>,
    ) -> Result<R<CursorPageVO<FacePersonVO, String>>, AppError> {
        let result = FaceService::search_person(
            &state.db,
            query,
            &state.encryption_key,
        )
        .await?;
        Ok(R::ok(result))
    }

    async fn get_person_info(
        State(state): State<Arc<AppState>>,
        Path(id): Path<String>,
    ) -> Result<R<FacePersonVO>, AppError> {
        let person_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result = FaceService::get_person_info(&state.db, person_id, &state.encryption_key).await?;
        Ok(R::ok(result))
    }

    async fn rename_person(
        State(state): State<Arc<AppState>>,
        Path(id): Path<String>,
        Json(req): Json<RenamePersonRequest>,
    ) -> Result<R<FacePersonVO>, AppError> {
        let person_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result =
            FaceService::rename_person(&state.db, &state.redis, person_id, req.new_name).await?;
        Ok(R::ok(result))
    }

    async fn get_person_photo(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Query(query): Query<PersonPageQuery>,
    ) -> Result<R<CursorPageVO<PhotoVO, i64>>, AppError> {
        let person_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let cursor = query.cursor.and_then(|s| s.parse().ok());
        let result = FaceService::get_person_photo(
            &state.db,
            &state.redis,
            user_id.0,
            person_id,
            cursor,
            query.size.unwrap_or(20),
            &state.encryption_key,
        )
        .await?;
        Ok(R::ok(result))
    }

    async fn merge_person(
        State(state): State<Arc<AppState>>,
        Json(req): Json<MergePersonRequest>,
    ) -> Result<R<FacePersonVO>, AppError> {
        let source_id: i64 = req
            .source_person_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的源人物ID"))?;
        let target_id: i64 = req
            .target_person_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的目标人物ID"))?;
        let result =
            FaceService::merge_person(&state.db, &state.redis, source_id, target_id).await?;
        Ok(R::ok(result))
    }

    async fn delete_person(
        State(state): State<Arc<AppState>>,
        Path(id): Path<String>,
    ) -> Result<R<bool>, AppError> {
        let person_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result = FaceService::delete_person(&state.db, person_id).await?;
        Ok(R::ok(result))
    }

    async fn get_photo_features(
        State(state): State<Arc<AppState>>,
        Path(photo_id): Path<String>,
    ) -> Result<R<Vec<FaceFeatureVO>>, AppError> {
        let photo_id: i64 = photo_id.parse().map_err(|_| AppError::bad_request("无效的照片ID"))?;
        let result = FeatureService::get_photo_features(&state.db, &state.redis, photo_id).await?;
        Ok(R::ok(result))
    }

    async fn change_face_belonging(
        State(state): State<Arc<AppState>>,
        Path((feature_id, person_id)): Path<(String, String)>,
    ) -> Result<R<()>, AppError> {
        let feature_id: i64 = feature_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的特征ID"))?;
        let person_id: i64 = person_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的人物ID"))?;
        FeatureService::change_face_belonging(&state.db, feature_id, person_id).await?;
        Ok(R::ok(()))
    }
}
