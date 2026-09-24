use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{delete, get},
};
use common_core::{Result, types::CursorPage};
use common_web::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedJson,
    extractors::ValidatedPath, extractors::ValidatedQuery,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    visual::{
        comment::CommentId,
        dto::comment::{CommentCursorPageParam, CommentPublishParam, CommentView},
        visual::VisualId,
    },
};

use crate::{services::comment_service::CommentService, state::VisualState};

pub struct CommentController;

impl ControllerRouter for CommentController {
    type State = VisualState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route(
                "/{visual_id}",
                get(Self::get_cursor_page).post(Self::publish),
            )
            .route("/{visual_id}/{comment_id}", delete(Self::delete))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl CommentController {
    /// 发布评论.
    async fn publish(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(visual_id): ValidatedPath<VisualId>,
        ValidatedJson(req): ValidatedJson<CommentPublishParam>,
    ) -> Result<R<CommentView>> {
        CommentService::publish(&state, user_id, visual_id, req)
            .await
            .to_r_ok()
    }
}

// 修改
impl CommentController {}

// 查询
impl CommentController {
    /// 游标查询评论列表.
    async fn get_cursor_page(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(visual_id): ValidatedPath<VisualId>,
        ValidatedQuery(req): ValidatedQuery<CommentCursorPageParam>,
    ) -> Result<R<CursorPage<CommentView, TimeIdCursor<CommentId>>>> {
        CommentService::get_cursor_page(&state, user_id, visual_id, req)
            .await
            .to_r_ok()
    }
}

// 删除
impl CommentController {
    ///删除评论.
    async fn delete(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath((_visual_id, comment_id)): ValidatedPath<(VisualId, CommentId)>,
    ) -> Result<R<()>> {
        CommentService::delete(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }
}
