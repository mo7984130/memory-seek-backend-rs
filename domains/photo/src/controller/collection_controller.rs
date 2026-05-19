use axum::extract::{Path, Query, State};
use axum::routing::{get, patch, post};
use axum::Extension;
use axum::Json;
use axum::Router;
use common::error::AppError;
use common::r::R;
use std::sync::Arc;

use common::models::UserId;
use crate::state::PhotoState;
use crate::models::collection::{
    BatchOperationResultVO, BatchPhotosDTO, CollectionCreateDTO, CollectionEditDTO,
    CollectionPhotoQuery, CollectionPhotoVO, CollectionVO,
};
use crate::models::photo::CursorPageVO;
use crate::services::collection_service::CollectionService;

pub struct CollectionController;

impl CollectionController {
    /// 构建收藏夹相关的路由
    ///
    /// # 返回
    /// 包含收藏夹 CRUD、照片管理等路由的 `Router`
    pub fn routes() -> Router<Arc<PhotoState>> {
        Router::new()
            .route("/", get(Self::get_list).post(Self::create))
            .route("/{id}", patch(Self::edit).delete(Self::delete))
            .route("/{id}/photos", get(Self::get_photos))
            .route("/{collection_id}/photos/{photo_id}", post(Self::add_photo).delete(Self::remove_photo))
            .route("/{collection_id}/photos/batch", post(Self::batch_add_photos).delete(Self::batch_remove_photos))
            .route("/photo/{photo_id}", get(Self::get_by_photo_id))
    }

    /// 获取当前用户的所有收藏夹列表
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    ///
    /// # 返回
    /// 返回收藏夹列表
    ///
    /// # 错误
    /// - `AppError`: 查询失败
    async fn get_list(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<Vec<CollectionVO>>, AppError> {
        let result = CollectionService::get_collection_list(
            &state,
            user_id.0,
        )
        .await?;
        Ok(R::ok(result))
    }

    /// 创建新收藏夹
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `dto`: 收藏夹创建信息（名称和描述）
    ///
    /// # 返回
    /// 返回新创建的收藏夹信息
    ///
    /// # 错误
    /// - `AppError`: 创建失败
    async fn create(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Json(dto): Json<CollectionCreateDTO>,
    ) -> Result<R<CollectionVO>, AppError> {
        let result =
            CollectionService::create_collection(&state, user_id.0, dto.name, dto.description)
                .await?;
        Ok(R::ok(result))
    }

    /// 编辑收藏夹信息
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `id`: 收藏夹 ID（路径参数）
    /// - `dto`: 收藏夹编辑信息（名称和描述）
    ///
    /// # 返回
    /// 返回更新后的收藏夹信息
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 收藏夹 ID 格式无效
    /// - `AppError`: 编辑失败或无权编辑
    async fn edit(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Json(dto): Json<CollectionEditDTO>,
    ) -> Result<R<CollectionVO>, AppError> {
        let collection_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result = CollectionService::edit_collection(
            &state,
            user_id.0,
            collection_id,
            dto.name,
            dto.description,
        )
        .await?;
        Ok(R::ok(result))
    }

    /// 删除收藏夹
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `id`: 收藏夹 ID（路径参数）
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 收藏夹 ID 格式无效
    /// - `AppError`: 删除失败或无权删除
    async fn delete(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
    ) -> Result<R<()>, AppError> {
        let collection_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        CollectionService::delete_collection(&state, user_id.0, collection_id).await?;
        Ok(R::ok(()))
    }

    /// 使用游标分页获取收藏夹中的照片列表
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `id`: 收藏夹 ID（路径参数）
    /// - `query`: 游标分页查询参数
    ///
    /// # 返回
    /// 返回游标分页的收藏夹照片列表
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 收藏夹 ID 格式无效
    /// - `AppError`: 查询失败或无权访问
    async fn get_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Query(query): Query<CollectionPhotoQuery>,
    ) -> Result<R<CursorPageVO<CollectionPhotoVO, String>>, AppError> {
        let collection_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result = CollectionService::get_collection_photos(
            &state,
            user_id.0,
            collection_id,
            query.cursor,
            query.size.unwrap_or(20),
        )
        .await?;
        Ok(R::ok(result))
    }

    /// 向收藏夹添加单张照片
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `collection_id`: 收藏夹 ID（路径参数）
    /// - `photo_id`: 照片 ID（路径参数）
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 收藏夹 ID 或照片 ID 格式无效
    /// - `AppError`: 添加失败或无权操作
    async fn add_photo(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path((collection_id, photo_id)): Path<(String, String)>,
    ) -> Result<R<()>, AppError> {
        let collection_id: i64 = collection_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的收藏夹ID"))?;
        let photo_id: i64 = photo_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的照片ID"))?;
        CollectionService::add_photo_to_collection(&state, user_id.0, collection_id, photo_id)
            .await?;
        Ok(R::ok(()))
    }

    /// 从收藏夹移除单张照片
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `collection_id`: 收藏夹 ID（路径参数）
    /// - `photo_id`: 照片 ID（路径参数）
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 收藏夹 ID 或照片 ID 格式无效
    /// - `AppError`: 移除失败或无权操作
    async fn remove_photo(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path((collection_id, photo_id)): Path<(String, String)>,
    ) -> Result<R<()>, AppError> {
        let collection_id: i64 = collection_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的收藏夹ID"))?;
        let photo_id: i64 = photo_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的照片ID"))?;
        CollectionService::remove_photo_from_collection(
            &state,
            user_id.0,
            collection_id,
            photo_id,
        )
        .await?;
        Ok(R::ok(()))
    }

    /// 批量向收藏夹添加照片
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `collection_id`: 收藏夹 ID（路径参数）
    /// - `dto`: 包含照片 ID 列表的请求体
    ///
    /// # 返回
    /// 返回批量操作结果，包含成功和失败的数量
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 收藏夹 ID 格式无效
    /// - `AppError`: 操作失败或无权操作
    async fn batch_add_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<String>,
        Json(dto): Json<BatchPhotosDTO>,
    ) -> Result<R<BatchOperationResultVO>, AppError> {
        let collection_id: i64 = collection_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的收藏夹ID"))?;
        let photo_ids: Vec<i64> = dto
            .photo_ids
            .into_iter()
            .filter_map(|id| id.parse().ok())
            .collect();
        let result = CollectionService::batch_add_photos_to_collection(
            &state,
            user_id.0,
            collection_id,
            photo_ids,
        )
        .await?;
        Ok(R::ok(result))
    }

    /// 批量从收藏夹移除照片
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `collection_id`: 收藏夹 ID（路径参数）
    /// - `dto`: 包含照片 ID 列表的请求体
    ///
    /// # 返回
    /// 返回批量操作结果，包含成功和失败的数量
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 收藏夹 ID 格式无效
    /// - `AppError`: 操作失败或无权操作
    async fn batch_remove_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(collection_id): Path<String>,
        Json(dto): Json<BatchPhotosDTO>,
    ) -> Result<R<BatchOperationResultVO>, AppError> {
        let collection_id: i64 = collection_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的收藏夹ID"))?;
        let photo_ids: Vec<i64> = dto
            .photo_ids
            .into_iter()
            .filter_map(|id| id.parse().ok())
            .collect();
        let result = CollectionService::batch_remove_photos_from_collection(
            &state,
            user_id.0,
            collection_id,
            photo_ids,
        )
        .await?;
        Ok(R::ok(result))
    }

    /// 获取包含指定照片的所有收藏夹 ID
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `photo_id`: 照片 ID（路径参数）
    ///
    /// # 返回
    /// 返回包含该照片的收藏夹 ID 列表
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 照片 ID 格式无效
    /// - `AppError`: 查询失败
    async fn get_by_photo_id(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(photo_id): Path<String>,
    ) -> Result<R<Vec<String>>, AppError> {
        let photo_id: i64 = photo_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的照片ID"))?;
        let result =
            CollectionService::find_collection_ids_by_photo(&state, user_id.0, photo_id)
                .await?;
        Ok(R::ok(result))
    }
}
