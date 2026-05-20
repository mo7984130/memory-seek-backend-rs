use chrono::{DateTime, Utc};
use common::error::AppError;
use sea_orm::TransactionTrait;

use crate::mappers::{CommentLikeMapper, CommentMapper};
use crate::models::comment::PhotoCommentVO;
use crate::models::photo::CursorPageVO;
use crate::state::PhotoState;

/// 评论分页参数限制
const COMMENT_PAGE_LIMIT_MIN: i64 = 1;
const COMMENT_PAGE_LIMIT_MAX: i64 = 100;

/// 热门评论配置
const HOT_COMMENT_MIN_LIKES: i32 = 5;
const HOT_COMMENT_MAX_COUNT: u64 = 3;

pub struct CommentService;

impl CommentService {
    /// 获取照片评论分页列表
    ///
    /// 首页会先显示热门评论（点赞数超过阈值的评论），
    /// 然后按时间倒序显示其他评论。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `photo_id`: 照片ID
    /// - `user_id`: 当前用户ID（用于判断点赞状态）
    /// - `cursor`: 游标时间点
    /// - `limit`: 每页数量
    ///
    /// # 返回
    /// 返回分页评论列表，包含用户点赞状态
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 分页参数超出范围
    pub async fn get_comment_page(
        state: &PhotoState,
        photo_id: i64,
        user_id: i64,
        cursor: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<CursorPageVO<PhotoCommentVO, DateTime<Utc>>, AppError> {
        // 校验 limit 参数
        if !(COMMENT_PAGE_LIMIT_MIN..=COMMENT_PAGE_LIMIT_MAX).contains(&limit) {
            return Err(AppError::bad_request("分页参数超出范围"));
        }
        let limit = limit as u64;
        let limit_usize = limit as usize;

        // 首页展示热门评论，翻页后不再重复
        let hot_comments = if cursor.is_none() {
            CommentMapper::query_hot_comments(&state.db, photo_id, HOT_COMMENT_MIN_LIKES, HOT_COMMENT_MAX_COUNT).await?
        } else {
            vec![]
        };

        // 查询时间线评论（排除已展示的热门评论）
        let hot_ids: Vec<i64> = hot_comments.iter().map(|c| c.id).collect();
        let time_comments = CommentMapper::query_by_photo_id(&state.db, photo_id, &hot_ids, cursor, limit).await?;

        // 判断是否有更多数据，并截取当前页
        let has_more = time_comments.len() > limit_usize;
        let time_comments: Vec<_> = time_comments.into_iter().take(limit_usize).collect();

        // 合并评论：热门在前，时间线在后
        let mut all_comments = hot_comments;
        all_comments.extend(time_comments);

        // 批量查询当前用户的点赞状态
        let comment_ids: Vec<i64> = all_comments.iter().map(|c| c.id).collect();
        let liked = CommentLikeMapper::query_by_user_and_comments(&state.db, user_id, comment_ids).await?;

        // 构建返回数据
        let records: Vec<PhotoCommentVO> = all_comments
            .iter()
            .map(|c| PhotoCommentVO {
                id: c.id.to_string(),
                user_id: c.user_id.to_string(),
                content: c.content.clone(),
                like_count: c.like_count,
                is_liked: liked.contains(&c.id),
                created_at: c.created_at.with_timezone(&Utc),
            })
            .collect();

        let next_cursor = all_comments.last().map(|c| c.created_at.with_timezone(&Utc));

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    /// 发布评论
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `photo_id`: 照片ID
    /// - `user_id`: 用户ID
    /// - `content`: 评论内容
    ///
    /// # 返回
    /// 返回创建的评论VO
    ///
    /// # 错误
    /// - `AppError`: 数据库插入失败
    pub async fn publish_comment(
        state: &PhotoState,
        photo_id: i64,
        user_id: i64,
        content: String,
    ) -> Result<PhotoCommentVO, AppError> {
        let comment = CommentMapper::insert(&state.db, photo_id, user_id, content).await?;

        Ok(PhotoCommentVO {
            id: comment.id.to_string(),
            user_id: user_id.to_string(),
            content: comment.content,
            like_count: 0,
            is_liked: false,
            created_at: comment.created_at.with_timezone(&Utc),
        })
    }

    /// 删除评论
    ///
    /// 只能删除自己的评论，同时删除评论的所有点赞记录。
    /// 使用事务保证原子性。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `comment_id`: 评论ID
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 无权限删除该评论
    /// - `AppError::InternalServerError`: 数据库事务失败
    pub async fn delete_comment(
        state: &PhotoState,
        user_id: i64,
        comment_id: i64,
    ) -> Result<(), AppError> {
        let comment = CommentMapper::query_by_id(&state.db, comment_id).await?;

        if comment.user_id != user_id {
            return Err(AppError::bad_request("无权限删除"));
        }

        state.db.transaction::<_, (), sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                CommentLikeMapper::delete_by_comment_id(txn, comment_id)
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                CommentMapper::delete_by_id(txn, comment_id)
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                Ok(())
            })
        }).await.map_err(|e| {
            tracing::error!(target:"logs", "删除评论失败: {:?}", e);
            AppError::InternalServerError
        })
    }

    /// 切换评论点赞状态
    ///
    /// 已点赞则取消，未点赞则添加，同时更新评论的点赞数。
    /// 使用事务保证原子性。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID
    /// - `comment_id`: 评论ID
    ///
    /// # 返回
    /// 返回点赞后的状态（`true` 为已点赞，`false` 为已取消）
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: 数据库事务失败
    pub async fn toggle_like(
        state: &PhotoState,
        user_id: i64,
        comment_id: i64,
    ) -> Result<bool, AppError> {
        let existing = CommentLikeMapper::query_by_user_and_comment(&state.db, user_id, comment_id).await?;

        if let Some(like) = existing {
            state.db.transaction::<_, (), sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    CommentLikeMapper::delete_by_id(txn, like.id)
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    CommentMapper::update_like_count(txn, comment_id, -1)
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    Ok(())
                })
            }).await.map_err(|e| {
                tracing::error!(target:"logs", "取消点赞失败: {:?}", e);
                AppError::InternalServerError
            })?;
            Ok(false)
        } else {
            state.db.transaction::<_, (), sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    CommentLikeMapper::insert(txn, comment_id, user_id)
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    CommentMapper::update_like_count(txn, comment_id, 1)
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    Ok(())
                })
            }).await.map_err(|e| {
                tracing::error!(target:"logs", "点赞失败: {:?}", e);
                AppError::InternalServerError
            })?;
            Ok(true)
        }
    }
}
