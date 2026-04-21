use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::Extension;
use axum::Json;
use axum::Router;
use common::error::AppError;
use common::r::R;
use std::sync::Arc;

use crate::middlewares::auth::UserId;
use crate::state::PhotoState;
use crate::models::comment::{CommentPageQuery, PhotoCommentVO, PublishCommentDTO};
use crate::models::photo::CursorPageVO;
use crate::services::comment_service::CommentService;

pub struct CommentController;

impl CommentController {
    pub fn routes() -> Router<Arc<PhotoState>> {
        Router::new()
            .route("/{comment_id}/like/toggle", post(Self::toggle_like))
            .route("/{id}", get(Self::get_list).post(Self::publish).delete(Self::delete))
    }

    async fn get_list(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Query(query): Query<CommentPageQuery>,
    ) -> Result<R<CursorPageVO<PhotoCommentVO, chrono::DateTime<chrono::Utc>>>, AppError> {
        let photo_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的照片ID"))?;
        let result = CommentService::get_comment_page(
            &state.db,
            photo_id,
            user_id.0,
            query.cursor,
            query.limit.unwrap_or(20),
        )
        .await?;
        Ok(R::ok(result))
    }

    async fn publish(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Json(dto): Json<PublishCommentDTO>,
    ) -> Result<R<PhotoCommentVO>, AppError> {
        let photo_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的照片ID"))?;
        let result =
            CommentService::publish_comment(&state.db, photo_id, user_id.0, dto.content).await?;
        Ok(R::ok(result))
    }

    async fn delete(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
    ) -> Result<R<()>, AppError> {
        let comment_id: i64 = id
            .parse()
            .map_err(|_| AppError::bad_request("无效的评论ID"))?;
        CommentService::delete_comment(&state.db, user_id.0, comment_id).await?;
        Ok(R::ok(()))
    }

    async fn toggle_like(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(comment_id): Path<String>,
    ) -> Result<R<bool>, AppError> {
        let comment_id: i64 = comment_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的评论ID"))?;
        let result = CommentService::toggle_like(&state.db, user_id.0, comment_id).await?;
        Ok(R::ok(result))
    }
}
