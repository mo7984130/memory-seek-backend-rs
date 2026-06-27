use std::{str::FromStr, sync::Arc};

use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use common::{
    Result,
    ext::{ResultErrExt, ResultRExt},
    metrics_group, metrics_success,
    models::CursorPage,
    r::R,
};
use entities::{auth::user::UserId, photo::photo::PhotoId};

use crate::{
    models::photo::PhotoResult,
    services::{
        photo_like_service::{PhotoLikeCursor, PhotoLikeService},
        photo_service::PhotoService,
    },
    state::PhotoState,
};
use common::traits::controller::ControllerRouter;

pub struct PhotoLikeController;

impl ControllerRouter for PhotoLikeController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route("/photos/{photo_id}/like", post(Self::like).delete(Self::unlike))
            .route("/photos/liked", get(Self::get_user_liked_photos))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 请求模型
#[derive(serde::Deserialize)]
struct LikedPhotosQuery {
    cursor: Option<String>,
    size: Option<u64>,
}

// 创建
impl PhotoLikeController {
    async fn like(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(photo_id): Path<String>,
    ) -> Result<R<()>> {
        metrics_group!("api_like_photo");

        let photo_id =
            PhotoId::from_str(&photo_id).trace_warn_bad_request(
                "invalid_photo_id",
                "无效的photo_id",
                "无效的photo_id",
            )?;

        PhotoLikeService::like(&state, user_id, photo_id).await?;

        metrics_success!("api_like_photo");
        Ok(()).to_r_ok()
    }
}

// 删除
impl PhotoLikeController {
    async fn unlike(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(photo_id): Path<String>,
    ) -> Result<R<()>> {
        metrics_group!("api_unlike_photo");

        let photo_id =
            PhotoId::from_str(&photo_id).trace_warn_bad_request(
                "invalid_photo_id",
                "无效的photo_id",
                "无效的photo_id",
            )?;

        PhotoLikeService::unlike(&state, user_id, photo_id).await?;

        metrics_success!("api_unlike_photo");
        Ok(()).to_r_ok()
    }
}

// 查询
impl PhotoLikeController {
    async fn get_user_liked_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Query(query): Query<LikedPhotosQuery>,
    ) -> Result<R<CursorPage<PhotoResult, String>>> {
        metrics_group!("api_get_user_liked_photos");

        let size = query.size.unwrap_or(20).min(100);

        // 查询用户点赞的照片ID列表
        let photo_ids = PhotoLikeService::get_user_liked_photos(
            &state,
            user_id,
            query.cursor.clone(),
            size + 1, // 多查一个用于判断has_more
        )
        .await?;

        // 构建CursorPage
        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        if photo_ids.is_empty() {
            metrics_success!("api_get_user_liked_photos");
            return Ok(CursorPage::empty()).to_r_ok();
        }

        // 加载照片详细信息
        let photos = PhotoService::load_photos_info(&state, user_id, &photo_ids).await?;

        // 生成next_cursor
        let next_cursor = if has_more {
            photos.last().and_then(|p| {
                let id = PhotoId::from_str(&p.id).ok()?;
                Some(PhotoLikeCursor { created_at: p.created_at, id }.encode())
            })
        } else {
            None
        };

        metrics_success!("api_get_user_liked_photos");
        Ok(CursorPage {
            records: photos,
            next_cursor,
            has_more,
        })
        .to_r_ok()
    }
}
