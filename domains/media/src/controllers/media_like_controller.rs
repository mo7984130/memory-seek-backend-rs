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
    media::{dto::media::MediaView, models::LikedMediasQuery, media::MediaId},
};

use crate::{services::media_like_service::MediaLikeService, state::MediaState};

pub struct MediaLikeController;

impl ControllerRouter for MediaLikeController {
    type State = MediaState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route(
                "/media/{media_id}/like",
                post(Self::like).delete(Self::unlike),
            )
            .route("/media/liked", get(Self::get_user_liked_medias))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl MediaLikeController {
    /// 为照片点赞.
    async fn like(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(media_id): ValidatedPath<MediaId>,
    ) -> Result<R<()>> {
        MediaLikeService::like(&state, user_id, media_id).await?;

        Ok(()).to_r_ok()
    }
}

// 删除
impl MediaLikeController {
    /// 取消照片点赞.
    async fn unlike(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(media_id): ValidatedPath<MediaId>,
    ) -> Result<R<()>> {
        MediaLikeService::unlike(&state, user_id, media_id).await?;

        Ok(()).to_r_ok()
    }
}

// 查询
impl MediaLikeController {
    /// 返回当前用户点赞过的照片分页.
    async fn get_user_liked_medias(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<LikedMediasQuery>,
    ) -> Result<R<CursorPage<MediaView, TimeIdCursor<MediaId>>>> {
        MediaLikeService::get_user_liked_medias(&state, user_id, req)
            .await
            .to_r_ok()
    }
}
