use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{delete, get},
};
use common::{
    Result,
    axum::{
        R,
        controller_router::ControllerRouter,
        ext::ToROkExt,
        extractors::{ValidatedJson, ValidatedPath, ValidatedQuery},
    },
    types::CursorPage,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    media::{
        comment::CommentId,
        dto::comment::{CommentCursorPageParam, CommentPublishParam, CommentView},
        media::MediaId,
    },
};

use crate::{services::comment_service::CommentService, state::MediaState};

pub struct CommentController;

impl ControllerRouter for CommentController {
    type State = MediaState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route(
                "/{media_id}",
                get(Self::get_cursor_page).post(Self::publish),
            )
            .route("/{media_id}/{comment_id}", delete(Self::delete))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl CommentController {
    /// 发布评论.
    async fn publish(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(media_id): ValidatedPath<MediaId>,
        ValidatedJson(req): ValidatedJson<CommentPublishParam>,
    ) -> Result<R<CommentView>> {
        CommentService::publish(&state, user_id, media_id, req)
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
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(media_id): ValidatedPath<MediaId>,
        ValidatedQuery(req): ValidatedQuery<CommentCursorPageParam>,
    ) -> Result<R<CursorPage<CommentView, TimeIdCursor<CommentId>>>> {
        CommentService::get_cursor_page(&state, user_id, media_id, req)
            .await
            .to_r_ok()
    }
}

// 删除
impl CommentController {
    ///删除评论.
    async fn delete(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath((_media_id, comment_id)): ValidatedPath<(MediaId, CommentId)>,
    ) -> Result<R<()>> {
        CommentService::delete(&state, user_id, comment_id)
            .await
            .to_r_ok()
    }
}
