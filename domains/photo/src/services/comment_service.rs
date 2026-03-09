use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use entities::{comment, comment_like};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};

use crate::models::comment::PhotoCommentVO;
use crate::models::photo::CursorPageVO;

pub struct CommentService;

impl CommentService {
    pub async fn get_comment_page(
        db: &DatabaseConnection,
        photo_id: i64,
        user_id: i64,
        cursor: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<CursorPageVO<PhotoCommentVO, DateTime<Utc>>, AppError> {
        let hot_comments = if cursor.is_none() {
            comment::Entity::find()
                .filter(comment::Column::PhotoId.eq(photo_id as i64))
                .filter(comment::Column::LikeCount.gt(5))
                .order_by_desc(comment::Column::LikeCount)
                .limit(3)
                .all(db)
                .await
                .map_internal_err("查询失败")?
        } else {
            vec![]
        };

        let hot_ids: Vec<i64> = hot_comments.iter().map(|c| c.id).collect();

        let limit = limit as u64 + 1;
        let mut query = comment::Entity::find()
            .filter(comment::Column::PhotoId.eq(photo_id as i64))
            .order_by_desc(comment::Column::CreatedAt)
            .limit(limit);

        if !hot_ids.is_empty() {
            query = query.filter(comment::Column::Id.is_not_in(hot_ids));
        }

        if let Some(c) = cursor {
            query = query.filter(comment::Column::CreatedAt.lt(c));
        }

        let time_comments = query.all(db).await.map_internal_err("查询失败")?;

        let has_more = time_comments.len() > limit as usize - 1;
        let time_comments: Vec<_> = time_comments.into_iter().take(limit as usize - 1).collect();

        let mut all_comments = hot_comments;
        all_comments.extend(time_comments);

        let comment_ids: Vec<i64> = all_comments.iter().map(|c| c.id).collect();

        let liked = if !comment_ids.is_empty() {
            comment_like::Entity::find()
                .filter(comment_like::Column::UserId.eq(user_id as i64))
                .filter(comment_like::Column::CommentId.is_in(comment_ids))
                .all(db)
                .await
                .map_internal_err("查询失败")?
                .into_iter()
                .map(|l| l.comment_id)
                .collect::<std::collections::HashSet<_>>()
        } else {
            std::collections::HashSet::new()
        };

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

    pub async fn publish_comment(
        db: &DatabaseConnection,
        photo_id: i64,
        user_id: i64,
        content: String,
    ) -> Result<PhotoCommentVO, AppError> {
        let now = Utc::now();
        let comment = comment::ActiveModel {
            photo_id: Set(photo_id as i64),
            user_id: Set(user_id as i64),
            content: Set(content),
            like_count: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let comment = comment.insert(db).await.map_internal_err("发布失败")?;

        Ok(PhotoCommentVO {
            id: comment.id.to_string(),
            user_id: user_id.to_string(),
            content: comment.content,
            like_count: 0,
            is_liked: false,
            created_at: now,
        })
    }

    pub async fn delete_comment(
        db: &DatabaseConnection,
        user_id: i64,
        comment_id: i64,
    ) -> Result<(), AppError> {
        let comment = comment::Entity::find_by_id(comment_id as i64)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::bad_request("评论不存在"))?;

        if comment.user_id != user_id as i64 {
            return Err(AppError::bad_request("无权限删除"));
        }

        comment_like::Entity::delete_many()
            .filter(comment_like::Column::CommentId.eq(comment_id as i64))
            .exec(db)
            .await
            .ok();

        comment::Entity::delete_by_id(comment_id as i64)
            .exec(db)
            .await
            .map_internal_err("删除失败")?;

        Ok(())
    }

    pub async fn toggle_like(
        db: &DatabaseConnection,
        user_id: i64,
        comment_id: i64,
    ) -> Result<bool, AppError> {
        let existing = comment_like::Entity::find()
            .filter(comment_like::Column::UserId.eq(user_id as i64))
            .filter(comment_like::Column::CommentId.eq(comment_id as i64))
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        if let Some(like) = existing {
            comment_like::Entity::delete_by_id(like.id)
                .exec(db)
                .await
                .map_internal_err("取消点赞失败")?;

            let comment = comment::Entity::find_by_id(comment_id as i64)
                .one(db)
                .await
                .map_internal_err("查询评论失败")?;
            if let Some(c) = comment {
                let mut active: comment::ActiveModel = c.into();
                active.like_count = Set((active.like_count.unwrap() - 1).max(0));
                let _ = active.update(db).await;
            }

            Ok(false)
        } else {
            let now = Utc::now();
            let like = comment_like::ActiveModel {
                comment_id: Set(comment_id as i64),
                user_id: Set(user_id as i64),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };

            like.insert(db).await.map_internal_err("点赞失败")?;

            let comment = comment::Entity::find_by_id(comment_id as i64)
                .one(db)
                .await
                .map_internal_err("查询评论失败")?;
            if let Some(c) = comment {
                let mut active: comment::ActiveModel = c.into();
                active.like_count = Set(active.like_count.unwrap() + 1);
                let _ = active.update(db).await;
            }

            Ok(true)
        }
    }
}
