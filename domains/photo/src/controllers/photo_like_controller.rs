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

/// 照片ID与点赞时间的映射，用于生成正确的分页游标
use sea_orm::entity::prelude::DateTimeUtc;
use std::collections::HashMap;

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

        let photo_id = PhotoId::from_str(&photo_id).trace_warn_bad_request(
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

        let photo_id = PhotoId::from_str(&photo_id).trace_warn_bad_request(
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

        // 查询用户点赞的照片ID列表和点赞时间
        let photo_ids_with_like_time = PhotoLikeService::get_user_liked_photos(
            &state,
            user_id,
            query.cursor.clone(),
            size + 1, // 多查一个用于判断has_more
        )
        .await?;

        // 构建CursorPage（只提取photo_id用于分页判断）
        let photo_ids: Vec<PhotoId> = photo_ids_with_like_time.iter().map(|(id, _)| *id).collect();
        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        if photo_ids.is_empty() {
            metrics_success!("api_get_user_liked_photos");
            return Ok(CursorPage::empty()).to_r_ok();
        }

        // 构建 photo_id -> like_created_at 的映射
        let like_time_map: HashMap<i64, DateTimeUtc> = photo_ids_with_like_time
            .into_iter()
            .take(photo_ids.len()) // 只取与 photo_ids 数量匹配的部分
            .map(|(id, created_at)| (id.0, created_at))
            .collect();

        // 加载照片详细信息
        let photos = PhotoService::load_photos_info(&state, user_id, &photo_ids).await?;

        // 生成next_cursor（使用点赞时间而非照片上传时间）
        let next_cursor = if has_more {
            photos.last().and_then(|p| {
                let id = PhotoId::from_str(&p.id).ok()?;
                // 使用点赞时间生成游标，确保分页正确
                let like_created_at = like_time_map.get(&id.0).copied()?;
                Some(
                    PhotoLikeCursor {
                        created_at: like_created_at,
                        id,
                    }
                    .encode(),
                )
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
