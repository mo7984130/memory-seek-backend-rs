use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::Extension;
use axum::Json;
use axum::Router;
use common::error::AppError;
use common::r::R;
use std::sync::Arc;

use common::models::UserId;
use crate::state::PhotoState;
use crate::models::face::{
    FaceFeatureVO, FacePersonSimpleVO, FacePersonVO, MergePersonRequest, PersonPageQuery,
    PersonSearchQuery, RenamePersonRequest,
};
use crate::models::photo::{CursorPageVO, PhotoVO};
use crate::services::face_service::FaceService;
use crate::services::feature_service::FeatureService;

pub struct FaceController;

impl FaceController {
    /// 构建人脸管理相关的路由
    ///
    /// # 返回
    /// 包含人物查询、重命名、合并、删除、特征管理等路由的 `Router`
    pub fn routes() -> Router<Arc<PhotoState>> {
        Router::new()
            .route("/person", get(Self::get_person_page))
            .route("/person/all", get(Self::get_all_person))
            .route("/person/search", get(Self::search_person))
            .route("/person/{id}", get(Self::get_person_info))
            .route("/person/{id}/name", post(Self::rename_person))
            .route("/person/{id}/photo", get(Self::get_person_photo))
            .route("/person/merge", post(Self::merge_person))
            .route("/person/{id}", delete(Self::delete_person))
            .route("/feature/{photo_id}", get(Self::get_photo_features))
            .route("/feature/{feature_id}/belonging/{person_id}", post(Self::change_face_belonging))
    }

    /// 使用游标分页获取人物列表
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `query`: 游标分页查询参数
    ///
    /// # 返回
    /// 返回游标分页的人物列表
    ///
    /// # 错误
    /// - `AppError`: 查询失败
    async fn get_person_page(
        State(state): State<Arc<PhotoState>>,
        Query(query): Query<PersonPageQuery>,
    ) -> Result<R<CursorPageVO<FacePersonVO, String>>, AppError> {
        let result =
            FaceService::get_person_page(&state, query)
                .await?;
        Ok(R::ok(result))
    }

    /// 获取所有人物的简要信息列表
    ///
    /// # 参数
    /// - `state`: 应用状态
    ///
    /// # 返回
    /// 返回所有人物的简要信息列表（用于下拉选择等场景）
    ///
    /// # 错误
    /// - `AppError`: 查询失败
    async fn get_all_person(
        State(state): State<Arc<PhotoState>>,
    ) -> Result<R<Vec<FacePersonSimpleVO>>, AppError> {
        let result = FaceService::get_all_person(&state).await?;
        Ok(R::ok(result))
    }

    /// 按名称搜索人物
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `query`: 搜索查询参数（包含关键词和分页信息）
    ///
    /// # 返回
    /// 返回匹配的人物列表（游标分页）
    ///
    /// # 错误
    /// - `AppError`: 查询失败
    async fn search_person(
        State(state): State<Arc<PhotoState>>,
        Query(query): Query<PersonSearchQuery>,
    ) -> Result<R<CursorPageVO<FacePersonVO, String>>, AppError> {
        let result = FaceService::search_person(
            &state,
            query,
        )
        .await?;
        Ok(R::ok(result))
    }

    /// 获取指定人物的详细信息
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `id`: 人物 ID（路径参数）
    ///
    /// # 返回
    /// 返回人物的详细信息
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 人物 ID 格式无效
    /// - `AppError`: 查询失败或人物不存在
    async fn get_person_info(
        State(state): State<Arc<PhotoState>>,
        Path(id): Path<String>,
    ) -> Result<R<FacePersonVO>, AppError> {
        let person_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result = FaceService::get_person_info(&state, person_id).await?;
        Ok(R::ok(result))
    }

    /// 重命名人物
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `id`: 人物 ID（路径参数）
    /// - `req`: 包含新名称的请求体
    ///
    /// # 返回
    /// 返回更新后的人物信息
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 人物 ID 格式无效
    /// - `AppError`: 重命名失败或人物不存在
    async fn rename_person(
        State(state): State<Arc<PhotoState>>,
        Path(id): Path<String>,
        Json(req): Json<RenamePersonRequest>,
    ) -> Result<R<FacePersonVO>, AppError> {
        let person_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result =
            FaceService::rename_person(&state, person_id, req.new_name).await?;
        Ok(R::ok(result))
    }

    /// 获取指定人物关联的照片列表
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `id`: 人物 ID（路径参数）
    /// - `query`: 游标分页查询参数
    ///
    /// # 返回
    /// 返回游标分页的人物照片列表
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 人物 ID 格式无效
    /// - `AppError`: 查询失败
    async fn get_person_photo(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Query(query): Query<PersonPageQuery>,
    ) -> Result<R<CursorPageVO<PhotoVO, i64>>, AppError> {
        let person_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let cursor = query.cursor.and_then(|s| s.parse().ok());
        let result = FaceService::get_person_photo(
            &state,
            user_id.0,
            person_id,
            cursor,
            query.size.unwrap_or(20),
        )
        .await?;
        Ok(R::ok(result))
    }

    /// 合并两个人物（将源人物的所有人脸合并到目标人物）
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `req`: 包含源人物 ID 和目标人物 ID 的请求体
    ///
    /// # 返回
    /// 返回合并后的人物信息
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 人物 ID 格式无效
    /// - `AppError`: 合并失败或人物不存在
    async fn merge_person(
        State(state): State<Arc<PhotoState>>,
        Json(req): Json<MergePersonRequest>,
    ) -> Result<R<FacePersonVO>, AppError> {
        let source_id: i64 = req
            .source_person_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的源人物ID"))?;
        let target_id: i64 = req
            .target_person_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的目标人物ID"))?;
        let result =
            FaceService::merge_person(&state, source_id, target_id).await?;
        Ok(R::ok(result))
    }

    /// 删除指定人物
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `id`: 人物 ID（路径参数）
    ///
    /// # 返回
    /// 返回删除是否成功
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 人物 ID 格式无效
    /// - `AppError`: 删除失败或人物不存在
    async fn delete_person(
        State(state): State<Arc<PhotoState>>,
        Path(id): Path<String>,
    ) -> Result<R<bool>, AppError> {
        let person_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的ID"))?;
        let result = FaceService::delete_person(&state, person_id).await?;
        Ok(R::ok(result))
    }

    /// 获取指定照片的所有人脸特征
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `photo_id`: 照片 ID（路径参数）
    ///
    /// # 返回
    /// 返回照片中检测到的人脸特征列表
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 照片 ID 格式无效
    /// - `AppError`: 查询失败
    async fn get_photo_features(
        State(state): State<Arc<PhotoState>>,
        Path(photo_id): Path<String>,
    ) -> Result<R<Vec<FaceFeatureVO>>, AppError> {
        let photo_id: i64 = photo_id.parse().map_err(|_| AppError::bad_request("无效的照片ID"))?;
        let result = FeatureService::get_photo_features(&state, photo_id).await?;
        Ok(R::ok(result))
    }

    /// 变更人脸特征所属的人物
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `feature_id`: 人脸特征 ID（路径参数）
    /// - `person_id`: 目标人物 ID（路径参数）
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 特征 ID 或人物 ID 格式无效
    /// - `AppError`: 操作失败或记录不存在
    async fn change_face_belonging(
        State(state): State<Arc<PhotoState>>,
        Path((feature_id, person_id)): Path<(String, String)>,
    ) -> Result<R<()>, AppError> {
        let feature_id: i64 = feature_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的特征ID"))?;
        let person_id: i64 = person_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的人物ID"))?;
        FeatureService::change_face_belonging(&state, feature_id, person_id).await?;
        Ok(R::ok(()))
    }
}
