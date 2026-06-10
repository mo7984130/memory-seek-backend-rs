use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::{Path, State},
    routing::post,
};
use common::{Result, ext::ResultRExt, r::R, traits::controller::Controller};
use entities::{auth::user::UserId, photo::comment::CommentId};

use crate::{services::comment_like_service::CommentLikeService, state::PhotoState};

pub struct CommentLikeController;

impl Controller for CommentLikeController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new().route("/{comment_id}/like", post(Self::like).delete(Self::unlike))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl CommentLikeController {
    async fn like(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(comment_id): Path<CommentId>,
    ) -> Result<R<()>> {
        CommentLikeService::like(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }
}

// 修改
impl CommentLikeController {}

// 查询
impl CommentLikeController {}

// 删除
impl CommentLikeController {
    async fn unlike(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(comment_id): Path<CommentId>,
    ) -> Result<R<()>> {
        CommentLikeService::unlike(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }
}
