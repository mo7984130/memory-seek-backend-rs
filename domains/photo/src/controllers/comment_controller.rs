use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::{delete, get},
};
use common::{Result, ext::ResultRExt, models::CursorPage, r::R, traits::controller::Controller};
use entities::{
    auth::user::UserId,
    photo::{comment::CommentId, photo::PhotoId},
};
use sea_orm::entity::prelude::DateTimeUtc;

use crate::{
    models::comment::{CommentCursorPageQuery, CommentPublishParam, PhotoCommentVO},
    services::comment_service::CommentService,
    state::PhotoState,
};

pub struct CommentController;

impl Controller for CommentController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route(
                "/photos/{photo_id}/comments",
                get(Self::get_cursor_page).post(Self::publish),
            )
            .route("/{comment_id}", delete(Self::delete))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl CommentController {
    async fn publish(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(photo_id): Path<PhotoId>,
        Json(param): Json<CommentPublishParam>,
    ) -> Result<R<PhotoCommentVO>> {
        CommentService::publish(&state, photo_id, user_id, param.content)
            .await
            .to_r_ok()
    }
}

// 修改
impl CommentController {}

// 查询
impl CommentController {
    async fn get_cursor_page(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(photo_id): Path<PhotoId>,
        Query(param): Query<CommentCursorPageQuery>,
    ) -> Result<R<CursorPage<PhotoCommentVO, DateTimeUtc>>> {
        CommentService::get_cursor_page(&state, photo_id, user_id, param.cursor, param.size)
            .await
            .to_r_ok()
    }
}

// 删除
impl CommentController {
    async fn delete(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(comment_id): Path<CommentId>,
    ) -> Result<R<()>> {
        CommentService::delete(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }
}
