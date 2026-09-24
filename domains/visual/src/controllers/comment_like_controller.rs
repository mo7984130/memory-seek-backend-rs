use std::sync::Arc;

use axum::{Extension, Router, extract::State, routing::post};
use common_core::Result;
use common_web::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedPath,
};
use types::{auth::user::UserId, visual::comment::CommentId};

use crate::{services::comment_like_service::CommentLikeService, state::VisualState};

pub struct CommentLikeController;

impl ControllerRouter for CommentLikeController {
    type State = VisualState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new().route("/{comment_id}/like", post(Self::like).delete(Self::unlike))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl CommentLikeController {
    /// 为评论点赞.
    async fn like(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(comment_id): ValidatedPath<CommentId>,
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
    /// 取消评论点赞.
    async fn unlike(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(comment_id): ValidatedPath<CommentId>,
    ) -> Result<R<()>> {
        CommentLikeService::unlike(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }
}
