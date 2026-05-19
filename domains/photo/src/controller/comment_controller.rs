use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::Extension;
use axum::Json;
use axum::Router;
use common::error::AppError;
use common::r::R;
use std::sync::Arc;

use common::models::UserId;
use crate::state::PhotoState;
use crate::models::comment::{CommentPageQuery, PhotoCommentVO, PublishCommentDTO};
use crate::models::photo::CursorPageVO;
use crate::services::comment_service::CommentService;

/// 照片评论控制器，处理评论的增删查及点赞操作
pub struct CommentController;

impl CommentController {
    /// 构建评论模块的路由表
    ///
    /// # 返回
    /// 返回评论相关的 axum Router，包含列表查询、发布、删除和点赞切换
    pub fn routes() -> Router<Arc<PhotoState>> {
        Router::new()
            .route("/{comment_id}/like/toggle", post(Self::toggle_like))
            .route("/{id}", get(Self::get_list).post(Self::publish).delete(Self::delete))
    }

    /// 获取照片的评论列表（游标分页）
    ///
    /// # 参数
    /// - `state`: 应用状态，包含数据库连接等依赖
    /// - `user_id`: 当前登录用户的 ID
    /// - `id`: 照片 ID（路径参数，字符串形式）
    /// - `query`: 分页查询参数，包含游标和每页数量
    ///
    /// # 返回
    /// 返回评论的游标分页列表，包含评论内容及用户信息
    ///
    /// # 错误
    /// - `AppError::bad_request`: 照片 ID 格式无效
    /// - `AppError`: 数据库查询失败等内部错误
    async fn get_list(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Query(query): Query<CommentPageQuery>,
    ) -> Result<R<CursorPageVO<PhotoCommentVO, chrono::DateTime<chrono::Utc>>>, AppError> {
        let photo_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的照片ID"))?;
        let result = CommentService::get_comment_page(
            &state,
            photo_id,
            user_id.0,
            query.cursor,
            query.limit.unwrap_or(20),
        )
        .await?;
        Ok(R::ok(result))
    }

    /// 发布照片评论
    ///
    /// # 参数
    /// - `state`: 应用状态，包含数据库连接等依赖
    /// - `user_id`: 当前登录用户的 ID
    /// - `id`: 照片 ID（路径参数，字符串形式）
    /// - `dto`: 评论内容，包含评论文本
    ///
    /// # 返回
    /// 返回新发布的评论信息
    ///
    /// # 错误
    /// - `AppError::bad_request`: 照片 ID 格式无效
    /// - `AppError`: 评论发布失败等内部错误
    async fn publish(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
        Json(dto): Json<PublishCommentDTO>,
    ) -> Result<R<PhotoCommentVO>, AppError> {
        let photo_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的照片ID"))?;
        let result =
            CommentService::publish_comment(&state, photo_id, user_id.0, dto.content).await?;
        Ok(R::ok(result))
    }

    /// 删除评论
    ///
    /// # 参数
    /// - `state`: 应用状态，包含数据库连接等依赖
    /// - `user_id`: 当前登录用户的 ID（仅评论作者可删除）
    /// - `id`: 评论 ID（路径参数，字符串形式）
    ///
    /// # 返回
    /// 返回 `()` 表示删除成功
    ///
    /// # 错误
    /// - `AppError::bad_request`: 评论 ID 格式无效
    /// - `AppError`: 无权删除或评论不存在等错误
    async fn delete(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
    ) -> Result<R<()>, AppError> {
        let comment_id: i64 = id
            .parse()
            .map_err(|_| AppError::bad_request("无效的评论ID"))?;
        CommentService::delete_comment(&state, user_id.0, comment_id).await?;
        Ok(R::ok(()))
    }

    /// 切换评论的点赞状态
    ///
    /// # 参数
    /// - `state`: 应用状态，包含数据库连接等依赖
    /// - `user_id`: 当前登录用户的 ID
    /// - `comment_id`: 评论 ID（路径参数，字符串形式）
    ///
    /// # 返回
    /// 返回 `bool`，`true` 表示已点赞，`false` 表示已取消点赞
    ///
    /// # 错误
    /// - `AppError::bad_request`: 评论 ID 格式无效
    /// - `AppError`: 数据库操作失败等内部错误
    async fn toggle_like(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(comment_id): Path<String>,
    ) -> Result<R<bool>, AppError> {
        let comment_id: i64 = comment_id
            .parse()
            .map_err(|_| AppError::bad_request("无效的评论ID"))?;
        let result = CommentService::toggle_like(&state, user_id.0, comment_id).await?;
        Ok(R::ok(result))
    }
}
