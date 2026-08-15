use audit::{AuditService, BehaviorRecordReq};
use std::sync::Arc;

use crate::{services::collection_photo_service::CollectionPhotoService, state::PhotoState};
use axum::{
    Extension, Router,
    extract::State,
    routing::{delete, get},
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
        behavior::UserBehaviorAction,
        collection::CollectionId,
        dto::collection::{
            CollectionBriefView, CollectionPhotoAddBatchParam, CollectionPhotoAddBatchResult,
            CollectionPhotoCursorPageParam, CollectionPhotoRemoveBatchParam,
            CollectionPhotoRemoveBatchResult,
        },
        dto::photo::PhotoView,
        models::PhotoIds,
        photo::PhotoId,
    },
};

pub struct CollectionPhotoController;

impl ControllerRouter for CollectionPhotoController {
    type State = PhotoState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route("/by-photo/{photo_id}", get(Self::get_collections_by_photo))
            .route(
                "/{collection_id}/photos",
                get(Self::get_cursor_page)
                    .post(Self::add_batch)
                    .delete(Self::remove_batch),
            )
            .route("/{collection_id}/photos/{photo_id}", delete(Self::remove))
    }

    fn public_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
    }
}

// 查询照片所属收藏夹
impl CollectionPhotoController {
    /// 查询指定照片所属的相册.
    async fn get_collections_by_photo(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(photo_id): ValidatedPath<PhotoId>,
    ) -> Result<R<Vec<CollectionBriefView>>> {
        CollectionPhotoService::get_collections_by_photo(&state, user_id, photo_id)
            .await
            .to_r_ok()
    }
}

// 创建
impl CollectionPhotoController {
    /// 批量将照片加入相册.
    async fn add_batch(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedJson(req): ValidatedJson<CollectionPhotoAddBatchParam>,
    ) -> Result<R<CollectionPhotoAddBatchResult>> {
        let result = CollectionPhotoService::add_photos(
            &state,
            user_id,
            collection_id,
            req.photo_ids.clone(),
        )
        .await?;

        // 行为审计：收藏照片（按批量传入的 photo_id 逐条记录）
        let ip_str = ip.map(|ip| ip.to_string());
        for pid in req.photo_ids.iter() {
            AuditService::record_behavior_best_effort(
                state.repo.database(),
                BehaviorRecordReq::new(user_id, UserBehaviorAction::Collect)
                    .with_photo(pid.0)
                    .with_detail(serde_json::json!({ "collectionId": collection_id.0 }))
                    .with_ip(ip_str.clone()),
            )
            .await;
        }

        Ok(result).to_r_ok()
    }
}

// 查询
impl CollectionPhotoController {
    /// 按游标返回相册中的照片.
    async fn get_cursor_page(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedQuery(req): ValidatedQuery<CollectionPhotoCursorPageParam>,
    ) -> Result<R<CursorPage<PhotoView, TimeIdCursor<PhotoId>>>> {
        CollectionPhotoService::get_photos(&state, user_id, collection_id, req)
            .await
            .to_r_ok()
    }
}

// 修改
impl CollectionPhotoController {}

// 删除
impl CollectionPhotoController {
    /// 从相册移除单张照片.
    async fn remove(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath((collection_id, photo_id)): ValidatedPath<(CollectionId, PhotoId)>,
    ) -> Result<R<()>> {
        CollectionPhotoService::remove_photos(
            &state,
            user_id,
            collection_id,
            PhotoIds::new(vec![photo_id]).unwrap(),
        )
        .await?;

        // 行为审计：取消收藏照片
        AuditService::record_behavior_best_effort(
            state.repo.database(),
            BehaviorRecordReq::new(user_id, UserBehaviorAction::Uncollect)
                .with_photo(photo_id.0)
                .with_detail(serde_json::json!({ "collectionId": collection_id.0 }))
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }

    /// 批量从相册移除照片.
    async fn remove_batch(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedJson(req): ValidatedJson<CollectionPhotoRemoveBatchParam>,
    ) -> Result<R<CollectionPhotoRemoveBatchResult>> {
        let result = CollectionPhotoService::remove_photos(
            &state,
            user_id,
            collection_id,
            req.photo_ids.clone(),
        )
        .await?;

        // 行为审计：取消收藏照片（按批量传入的 photo_id 逐条记录）
        let ip_str = ip.map(|ip| ip.to_string());
        for pid in req.photo_ids.iter() {
            AuditService::record_behavior_best_effort(
                state.repo.database(),
                BehaviorRecordReq::new(user_id, UserBehaviorAction::Uncollect)
                    .with_photo(pid.0)
                    .with_detail(serde_json::json!({ "collectionId": collection_id.0 }))
                    .with_ip(ip_str.clone()),
            )
            .await;
        }

        Ok(result).to_r_ok()
    }
}
