use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{get, post},
};
use common::{
    Result,
    axum::{
        R,
        controller_router::ControllerRouter,
        ext::ToROkExt,
        extractors::{ValidatedPath, ValidatedQuery},
    },
    types::CursorPage,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{dto::photo::PhotoView, models::LikedPhotosQuery, photo::PhotoId},
};

use crate::{services::photo_like_service::PhotoLikeService, state::PhotoState};

pub struct PhotoLikeController;

impl ControllerRouter for PhotoLikeController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route(
                "/photos/{photo_id}/like",
                post(Self::like).delete(Self::unlike),
            )
            .route("/photos/liked", get(Self::get_user_liked_photos))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl PhotoLikeController {
    /// 为照片点赞.
    async fn like(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(photo_id): ValidatedPath<PhotoId>,
    ) -> Result<R<()>> {
        PhotoLikeService::like(&state, user_id, photo_id).await?;

        Ok(()).to_r_ok()
    }
}

// 删除
impl PhotoLikeController {
    /// 取消照片点赞.
    async fn unlike(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(photo_id): ValidatedPath<PhotoId>,
    ) -> Result<R<()>> {
        PhotoLikeService::unlike(&state, user_id, photo_id).await?;

        Ok(()).to_r_ok()
    }
}

// 查询
impl PhotoLikeController {
    /// 返回当前用户点赞过的照片分页.
    async fn get_user_liked_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<LikedPhotosQuery>,
    ) -> Result<R<CursorPage<PhotoView, TimeIdCursor<PhotoId>>>> {
        PhotoLikeService::get_user_liked_photos(&state, user_id, req)
            .await
            .to_r_ok()
    }
}
