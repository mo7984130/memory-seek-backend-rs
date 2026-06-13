use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::{Path, State},
    routing::{delete, get, post},
};
use common::{
    Result,
    ext::ResultRExt,
    extractors::{ValidatedJson, ValidatedQuery},
    models::CursorPage,
    r::R,
    traits::controller::ControllerRouter,
};
use entities::{
    auth::user::UserId,
    photo::{comment::CommentId, photo::PhotoId},
};
use sea_orm::entity::prelude::DateTimeUtc;

use crate::{
    models::comment::{CommentCursorPageParam, CommentPublishParam, PhotoCommentResult},
    services::{comment_like_service::CommentLikeService, comment_service::CommentService},
    state::PhotoState,
};

pub struct CommentController;

impl ControllerRouter for CommentController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route(
                "/{photo_id}",
                get(Self::get_cursor_page).post(Self::publish),
            )
            .route("/{photo_id}/{comment_id}", delete(Self::delete))
            .route(
                "/{photo_id}/{comment_id}/like",
                post(Self::like).delete(Self::unlike),
            )
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
        ValidatedJson(param): ValidatedJson<CommentPublishParam>,
    ) -> Result<R<PhotoCommentResult>> {
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
        ValidatedQuery(param): ValidatedQuery<CommentCursorPageParam>,
    ) -> Result<R<CursorPage<PhotoCommentResult, DateTimeUtc>>> {
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
        Path((photo_id, comment_id)): Path<(PhotoId, CommentId)>,
    ) -> Result<R<()>> {
        let _ = photo_id;
        CommentService::delete(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }
}

// 点赞
impl CommentController {
    async fn like(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path((photo_id, comment_id)): Path<(PhotoId, CommentId)>,
    ) -> Result<R<()>> {
        let _ = photo_id;
        CommentLikeService::like(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }

    async fn unlike(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path((photo_id, comment_id)): Path<(PhotoId, CommentId)>,
    ) -> Result<R<()>> {
        let _ = photo_id;
        CommentLikeService::unlike(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }
}
