use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{get, post},
};
use common::{
    Result,
    ext::ResultRExt,
    extractors::{ValidatedPath, ValidatedQuery},
    models::{CursorPage, TimeIdCursor},
    r::R,
};
use types::{
    auth::user::UserId,
    photo::{models::LikedPhotosQuery, photo::PhotoId},
};

use crate::{
    models::photo::PhotoResult, services::photo_like_service::PhotoLikeService, state::PhotoState,
};
use common::traits::controller::ControllerRouter;

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
    async fn get_user_liked_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(query): ValidatedQuery<LikedPhotosQuery>,
    ) -> Result<R<CursorPage<PhotoResult, String>>> {
        let size = query.size.unwrap_or(20).min(100);
        let cursor = query
            .cursor
            .map(TimeIdCursor::<PhotoId>::decode)
            .transpose()?;

        let result = PhotoLikeService::get_user_liked_photos(&state, user_id, cursor, size).await?;

        Ok(result).to_r_ok()
    }
}
