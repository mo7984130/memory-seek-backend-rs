use std::sync::Arc;

use crate::{services::collection_visual_service::CollectionVisualService, state::VisualState};
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
    visual::{
        collection::CollectionId,
        dto::collection::{
            CollectionBriefView, CollectionVisualAddBatchParam, CollectionVisualAddBatchResult,
            CollectionVisualCursorPageParam, CollectionVisualRemoveBatchParam,
            CollectionVisualRemoveBatchResult,
        },
        dto::visual::VisualView,
        models::VisualIds,
        visual::VisualId,
    },
};

pub struct CollectionVisualController;

impl ControllerRouter for CollectionVisualController {
    type State = VisualState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route(
                "/by-visual/{visual_id}",
                get(Self::get_collections_by_visual),
            )
            .route(
                "/{collection_id}/visuals",
                get(Self::get_cursor_page)
                    .post(Self::add_batch)
                    .delete(Self::remove_batch),
            )
            .route("/{collection_id}/visuals/{visual_id}", delete(Self::remove))
    }

    fn public_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
    }
}

// 查询影像所属收藏夹
impl CollectionVisualController {
    /// 查询指定影像所属的相册.
    async fn get_collections_by_visual(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(visual_id): ValidatedPath<VisualId>,
    ) -> Result<R<Vec<CollectionBriefView>>> {
        CollectionVisualService::get_collections_by_visual(&state, user_id, visual_id)
            .await
            .to_r_ok()
    }
}

// 创建
impl CollectionVisualController {
    /// 批量将影像加入相册.
    async fn add_batch(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedJson(req): ValidatedJson<CollectionVisualAddBatchParam>,
    ) -> Result<R<CollectionVisualAddBatchResult>> {
        let result = CollectionVisualService::add_visuals(
            &state,
            user_id,
            collection_id,
            req.visual_ids.clone(),
        )
        .await?;

        Ok(result).to_r_ok()
    }
}

// 查询
impl CollectionVisualController {
    /// 按游标返回相册中的影像.
    async fn get_cursor_page(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedQuery(req): ValidatedQuery<CollectionVisualCursorPageParam>,
    ) -> Result<R<CursorPage<VisualView, TimeIdCursor<VisualId>>>> {
        CollectionVisualService::get_visuals(&state, user_id, collection_id, req)
            .await
            .to_r_ok()
    }
}

// 修改
impl CollectionVisualController {}

// 删除
impl CollectionVisualController {
    /// 从相册移除单张影像.
    async fn remove(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath((collection_id, visual_id)): ValidatedPath<(CollectionId, VisualId)>,
    ) -> Result<R<()>> {
        CollectionVisualService::remove_visuals(
            &state,
            user_id,
            collection_id,
            VisualIds::new(vec![visual_id]).unwrap(),
        )
        .await?;

        Ok(()).to_r_ok()
    }

    /// 批量从相册移除影像.
    async fn remove_batch(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedJson(req): ValidatedJson<CollectionVisualRemoveBatchParam>,
    ) -> Result<R<CollectionVisualRemoveBatchResult>> {
        let result = CollectionVisualService::remove_visuals(
            &state,
            user_id,
            collection_id,
            req.visual_ids.clone(),
        )
        .await?;

        Ok(result).to_r_ok()
    }
}
