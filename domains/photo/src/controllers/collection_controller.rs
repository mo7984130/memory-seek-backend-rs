use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{get, patch},
};
use common::{
    Result,
    axum::{
        R,
        controller_router::ControllerRouter,
        ext::ToROkExt,
        extractors::{ValidatedJson, ValidatedPath},
    },
};
use types::{
    auth::user::UserId,
    photo::{
        collection::CollectionId,
        dto::collection::{CollectionCreateParam, CollectionUpdateParam, CollectionView},
    },
};

use crate::{services::collection_service::CollectionService, state::PhotoState};

pub struct CollectionController;

impl ControllerRouter for CollectionController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route("/", get(Self::get_list).post(Self::create))
            .route("/{id}", patch(Self::update_info).delete(Self::delete))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl CollectionController {
    /// 创建收藏夹
    async fn create(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<CollectionCreateParam>,
    ) -> Result<R<CollectionView>> {
        CollectionService::create_collection(&state, user_id, req)
            .await
            .to_r_ok()
    }
}

// 查询
impl CollectionController {
    /// 获取收藏夹列表.
    async fn get_list(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<Vec<CollectionView>>> {
        CollectionService::get_collection_list(&state, user_id)
            .await
            .to_r_ok()
    }
}

// 修改
impl CollectionController {
    /// 更新收藏夹信息.
    async fn update_info(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
        ValidatedJson(req): ValidatedJson<CollectionUpdateParam>,
    ) -> Result<R<()>> {
        CollectionService::update_collection_info(&state, user_id, collection_id, req)
            .await
            .to_r_ok()
    }
}

// 删除
impl CollectionController {
    /// 删除收藏夹.
    async fn delete(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(collection_id): ValidatedPath<CollectionId>,
    ) -> Result<R<()>> {
        CollectionService::delete_collection(&state, user_id, collection_id)
            .await
            .to_r_ok()
    }
}
