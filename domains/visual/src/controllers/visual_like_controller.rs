use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{get, post},
};
use common_core::{Result, types::CursorPage};
use common_web::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedPath,
    extractors::ValidatedQuery,
};
use types_core::cursor::TimeIdCursor;
use types_identity::auth::user::UserId;
use types_visual::{dto::visual::VisualView, models::LikedVisualsQuery, visual::VisualId};

use crate::{services::visual_like_service::VisualLikeService, state::VisualState};

pub struct VisualLikeController;

impl ControllerRouter for VisualLikeController {
    type State = VisualState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route(
                "/visual/{visual_id}/like",
                post(Self::like).delete(Self::unlike),
            )
            .route("/visual/liked", get(Self::get_user_liked_visuals))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl VisualLikeController {
    /// 为影像点赞.
    async fn like(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(visual_id): ValidatedPath<VisualId>,
    ) -> Result<R<()>> {
        VisualLikeService::like(&state, user_id, visual_id).await?;

        Ok(()).to_r_ok()
    }
}

// 删除
impl VisualLikeController {
    /// 取消影像点赞.
    async fn unlike(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(visual_id): ValidatedPath<VisualId>,
    ) -> Result<R<()>> {
        VisualLikeService::unlike(&state, user_id, visual_id).await?;

        Ok(()).to_r_ok()
    }
}

// 查询
impl VisualLikeController {
    /// 返回当前用户点赞过的影像分页.
    async fn get_user_liked_visuals(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<LikedVisualsQuery>,
    ) -> Result<R<CursorPage<VisualView, TimeIdCursor<VisualId>>>> {
        VisualLikeService::get_user_liked_visuals(&state, user_id, req)
            .await
            .to_r_ok()
    }
}
