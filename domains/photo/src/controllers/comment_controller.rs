use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{delete, get, post},
};
use common::{
    Result,
    ext::ResultRExt,
    extractors::{OptionalClientIp, ValidatedJson, ValidatedPath, ValidatedQuery},
    models::CursorPage,
    r::R,
    traits::controller::ControllerRouter,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{
        behavior::{BehaviorTargetType, UserBehaviorAction},
        comment::CommentId,
        dto::comment::{CommentCursorPageParam, CommentPublishParam, CommentView},
        photo::PhotoId,
    },
};

use crate::{
    services::{
        behavior_service::{BehaviorRecordReq, BehaviorService},
        comment_like_service::CommentLikeService,
        comment_service::CommentService,
    },
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
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath(photo_id): ValidatedPath<PhotoId>,
        ValidatedJson(req): ValidatedJson<CommentPublishParam>,
    ) -> Result<R<CommentView>> {
        let comment = CommentService::publish(&state, user_id, photo_id, req).await?;

        // 行为审计：发布评论
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::CommentPublish)
                .with_photo(photo_id.0)
                .with_detail(serde_json::json!({ "commentId": comment.id.0 }))
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(comment).to_r_ok()
    }
}

// 修改
impl CommentController {}

// 查询
impl CommentController {
    async fn get_cursor_page(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(photo_id): ValidatedPath<PhotoId>,
        ValidatedQuery(req): ValidatedQuery<CommentCursorPageParam>,
    ) -> Result<R<CursorPage<CommentView, TimeIdCursor<CommentId>>>> {
        CommentService::get_cursor_page(&state, user_id, photo_id, req)
            .await
            .to_r_ok()
    }
}

// 删除
impl CommentController {
    async fn delete(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath((photo_id, comment_id)): ValidatedPath<(PhotoId, CommentId)>,
    ) -> Result<R<()>> {
        CommentService::delete(&state, user_id, comment_id).await?;

        // 行为审计：删除评论
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::CommentDelete)
                .with_photo(photo_id.0)
                .with_detail(serde_json::json!({ "commentId": comment_id.0 }))
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }
}

// 点赞
impl CommentController {
    async fn like(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath((photo_id, comment_id)): ValidatedPath<(PhotoId, CommentId)>,
    ) -> Result<R<()>> {
        CommentLikeService::like(&state, user_id, comment_id).await?;

        // 行为审计：点赞评论
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::CommentLike)
                .with_target(BehaviorTargetType::Comment, comment_id.0)
                .with_detail(serde_json::json!({ "photoId": photo_id.0 }))
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }

    async fn unlike(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath((photo_id, comment_id)): ValidatedPath<(PhotoId, CommentId)>,
    ) -> Result<R<()>> {
        CommentLikeService::unlike(&state, user_id, comment_id).await?;

        // 行为审计：取消点赞评论
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::CommentUnlike)
                .with_target(BehaviorTargetType::Comment, comment_id.0)
                .with_detail(serde_json::json!({ "photoId": photo_id.0 }))
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }
}
