use chrono::{DateTime, Utc};
use common::error::AppError;
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::mappers::{CommentMapper, CommentLikeMapper};
use crate::models::comment::PhotoCommentVO;
use crate::models::photo::CursorPageVO;

pub struct CommentService;

impl CommentService {
    /// 获取照片评论分页列表
    /// 
    /// 首页会先显示热门评论（点赞数超过阈值的评论）
    /// 然后按时间倒序显示其他评论
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    /// - `user_id`: 当前用户ID（用于判断点赞状态）
    /// - `cursor`: 游标时间点
    /// - `limit`: 每页数量
    /// 
    /// # 返回
    /// 返回分页评论列表，包含用户点赞状态
    pub async fn get_comment_page(
        db: &DatabaseConnection,
        photo_id: i64,
        user_id: i64,
        cursor: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<CursorPageVO<PhotoCommentVO, DateTime<Utc>>, AppError> {
        let hot_comments = if cursor.is_none() {
            CommentMapper::find_hot_comments(db, photo_id, 5, 3).await?
        } else {
            vec![]
        };

        let hot_ids: Vec<i64> = hot_comments.iter().map(|c| c.id).collect();

        let time_comments = CommentMapper::find_by_photo_id_excluding_ids(db, photo_id, hot_ids, cursor, limit as u64).await?;

        let has_more = time_comments.len() > limit as usize;
        let time_comments: Vec<_> = time_comments.into_iter().take(limit as usize).collect();

        let mut all_comments = hot_comments;
        all_comments.extend(time_comments);

        let comment_ids: Vec<i64> = all_comments.iter().map(|c| c.id).collect();

        let liked = CommentLikeMapper::find_by_user_and_comments(db, user_id, comment_ids).await?;

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
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    /// - `user_id`: 用户ID
    /// - `content`: 评论内容
    /// 
    /// # 返回
    /// 返回创建的评论VO
    pub async fn publish_comment(
        db: &DatabaseConnection,
        photo_id: i64,
        user_id: i64,
        content: String,
    ) -> Result<PhotoCommentVO, AppError> {
        let comment = CommentMapper::insert(db, photo_id, user_id, content).await?;

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
    /// 只能删除自己的评论
    /// 同时删除评论的所有点赞记录
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `comment_id`: 评论ID
    /// 
    /// # 错误
    /// - 无权限删除返回400错误
    pub async fn delete_comment(
        db: &DatabaseConnection,
        user_id: i64,
        comment_id: i64,
    ) -> Result<(), AppError> {
        let comment = CommentMapper::find_by_id(db, comment_id).await?;

        if comment.user_id != user_id {
            return Err(AppError::bad_request("无权限删除"));
        }

        db.transaction::<_, (), sea_orm::DbErr>(|txn| {
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
    /// 已点赞则取消，未点赞则添加
    /// 同时更新评论的点赞数
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `comment_id`: 评论ID
    /// 
    /// # 返回
    /// 返回点赞后的状态（true为已点赞，false为已取消）
    pub async fn toggle_like(
        db: &DatabaseConnection,
        user_id: i64,
        comment_id: i64,
    ) -> Result<bool, AppError> {
        let existing = CommentLikeMapper::find_by_user_and_comment(db, user_id, comment_id).await?;

        if let Some(like) = existing {
            db.transaction::<_, (), sea_orm::DbErr>(|txn| {
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
            db.transaction::<_, (), sea_orm::DbErr>(|txn| {
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
