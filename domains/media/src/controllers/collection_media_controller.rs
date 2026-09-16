use std::sync::Arc;

use crate::{services::collection_media_service::CollectionMediaService, state::MediaState};
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
        collection::CollectionId,
        dto::collection::{
            CollectionBriefView, CollectionMediaAddBatchParam, CollectionMediaAddBatchResult,
            CollectionMediaCursorPageParam, CollectionMediaRemoveBatchParam,
            CollectionMediaRemoveBatchResult,
        },
        dto::media::MediaView,
        media::MediaId,
        models::MediaIds,
    },
};

pub struct CollectionMediaController;

impl ControllerRouter for CollectionMediaController {
    type State = MediaState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route("/by-media/{media_id}", get(Self::get_collections_by_media))
            .route(
                "/{collection_id}/medias",
                get(Self::get_cursor_page)
                    .post(Self::add_batch)
                    .delete(Self::remove_batch),
            )
            .route("/{collection_id}/medias/{media_id}", delete(Self::remove))
    }

    fn public_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
    }
}

// 查询媒体所属收藏夹
impl CollectionMediaController {
    /// 查询指定媒体所属的相册.
    async fn get_collections_by_media(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(media_id): ValidatedPath<MediaId>,
    ) -> Result<R<Vec<CollectionBriefView>>> {
        CollectionMediaService::get_collections_by_media(&state, user_id, media_id)
            .await
            .to_r_ok()
    }
}

// 创建
impl CollectionMediaController {
    /// 批量将媒体加入相册.
    async fn add_batch(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedJson(req): ValidatedJson<CollectionMediaAddBatchParam>,
    ) -> Result<R<CollectionMediaAddBatchResult>> {
        let result = CollectionMediaService::add_medias(
            &state,
            user_id,
            collection_id,
            req.media_ids.clone(),
        )
        .await?;

        Ok(result).to_r_ok()
    }
}

// 查询
impl CollectionMediaController {
    /// 按游标返回相册中的媒体.
    async fn get_cursor_page(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedQuery(req): ValidatedQuery<CollectionMediaCursorPageParam>,
    ) -> Result<R<CursorPage<MediaView, TimeIdCursor<MediaId>>>> {
        CollectionMediaService::get_medias(&state, user_id, collection_id, req)
            .await
            .to_r_ok()
    }
}

// 修改
impl CollectionMediaController {}

// 删除
impl CollectionMediaController {
    /// 从相册移除单张媒体.
    async fn remove(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath((collection_id, media_id)): ValidatedPath<(CollectionId, MediaId)>,
    ) -> Result<R<()>> {
        CollectionMediaService::remove_medias(
            &state,
            user_id,
            collection_id,
            MediaIds::new(vec![media_id]).unwrap(),
        )
        .await?;

        Ok(()).to_r_ok()
    }

    /// 批量从相册移除媒体.
    async fn remove_batch(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedJson(req): ValidatedJson<CollectionMediaRemoveBatchParam>,
    ) -> Result<R<CollectionMediaRemoveBatchResult>> {
        let result = CollectionMediaService::remove_medias(
            &state,
            user_id,
            collection_id,
            req.media_ids.clone(),
        )
        .await?;

        Ok(result).to_r_ok()
    }
}
