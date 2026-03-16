use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use entities::{comment, comment_like};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};

use std::collections::HashSet;

pub struct CommentMapper;

impl CommentMapper {
    /// 根据ID查询单条评论
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `id`: 评论ID
    /// 
    /// # 返回
    /// - 成功: 返回评论模型
    /// - 失败: 评论不存在返回404错误
    pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<comment::Model, AppError> {
        comment::Entity::find_by_id(id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::not_found("评论不存在"))
    }

    /// 游标分页查询照片的评论
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    /// - `cursor`: 游标时间点
    /// - `limit`: 每页数量
    /// 
    /// # 返回
    /// 返回评论列表，按创建时间倒序
    pub async fn find_by_photo_id(
        db: &DatabaseConnection,
        photo_id: i64,
        cursor: Option<DateTime<Utc>>,
        limit: u64,
    ) -> Result<Vec<comment::Model>, AppError> {
        let l = limit + 1;
        let mut query = comment::Entity::find()
            .filter(comment::Column::PhotoId.eq(photo_id))
            .order_by_desc(comment::Column::CreatedAt)
            .limit(l);

        if let Some(c) = cursor {
            query = query.filter(comment::Column::CreatedAt.lt(c));
        }

        query.all(db).await.map_internal_err("查询失败")
    }

    /// 查询照片的热门评论
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    /// - `min_likes`: 最小点赞数阈值
    /// - `limit`: 返回数量限制
    /// 
    /// # 返回
    /// 返回点赞数超过阈值的评论，按点赞数倒序
    pub async fn find_hot_comments(
        db: &DatabaseConnection,
        photo_id: i64,
        min_likes: i32,
        limit: u64,
    ) -> Result<Vec<comment::Model>, AppError> {
        comment::Entity::find()
            .filter(comment::Column::PhotoId.eq(photo_id))
            .filter(comment::Column::LikeCount.gt(min_likes))
            .order_by_desc(comment::Column::LikeCount)
            .limit(limit)
            .all(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 游标分页查询照片评论，排除指定ID
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    /// - `exclude_ids`: 要排除的评论ID列表
    /// - `cursor`: 游标时间点
    /// - `limit`: 每页数量
    /// 
    /// # 返回
    /// 返回评论列表（排除指定ID），按创建时间倒序
    pub async fn find_by_photo_id_excluding_ids(
        db: &DatabaseConnection,
        photo_id: i64,
        exclude_ids: Vec<i64>,
        cursor: Option<DateTime<Utc>>,
        limit: u64,
    ) -> Result<Vec<comment::Model>, AppError> {
        let l = limit + 1;
        let mut query = comment::Entity::find()
            .filter(comment::Column::PhotoId.eq(photo_id))
            .order_by_desc(comment::Column::CreatedAt)
            .limit(l);

        if !exclude_ids.is_empty() {
            query = query.filter(comment::Column::Id.is_not_in(exclude_ids));
        }

        if let Some(c) = cursor {
            query = query.filter(comment::Column::CreatedAt.lt(c));
        }

        query.all(db).await.map_internal_err("查询失败")
    }

    /// 发布新评论
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    /// - `user_id`: 用户ID
    /// - `content`: 评论内容
    /// 
    /// # 返回
    /// 返回创建的评论模型
    pub async fn insert(
        db: &DatabaseConnection,
        photo_id: i64,
        user_id: i64,
        content: String,
    ) -> Result<comment::Model, AppError> {
        let now = Utc::now();
        let comment = comment::ActiveModel {
            photo_id: Set(photo_id),
            user_id: Set(user_id),
            content: Set(content),
            like_count: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        comment.insert(db).await.map_internal_err("发布失败")
    }

    /// 删除评论
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 评论ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        comment::Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_internal_err("删除失败")?;
        Ok(())
    }

    /// 更新评论点赞数
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 评论ID
    /// - `delta`: 点赞数变化量（正数增加，负数减少）
    /// 
    /// # 返回
    /// 返回更新后的评论模型，点赞数最小为0
    pub async fn update_like_count<C: ConnectionTrait>(db: &C, id: i64, delta: i32) -> Result<comment::Model, AppError> {
        let existing = comment::Entity::find_by_id(id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::not_found("评论不存在"))?;
        let mut active: comment::ActiveModel = existing.into();
        let new_count = (active.like_count.as_ref() + delta).max(0);
        active.like_count = Set(new_count);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await.map_internal_err("更新失败")
    }

    /// 查询照片的所有评论ID
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `photo_id`: 照片ID
    /// 
    /// # 返回
    /// 返回评论ID列表
    pub async fn find_ids_by_photo_id<C: ConnectionTrait>(db: &C, photo_id: i64) -> Result<Vec<i64>, AppError> {
        let comments = comment::Entity::find()
            .filter(comment::Column::PhotoId.eq(photo_id))
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        Ok(comments.iter().map(|c| c.id).collect())
    }

    /// 删除照片的所有评论
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `photo_id`: 照片ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_photo_id<C: ConnectionTrait>(db: &C, photo_id: i64) -> Result<(), AppError> {
        comment::Entity::delete_many()
            .filter(comment::Column::PhotoId.eq(photo_id))
            .exec(db)
            .await
            .map_internal_err("删除评论失败")?;
        Ok(())
    }
}

pub struct CommentLikeMapper;

impl CommentLikeMapper {
    /// 查询用户对评论的点赞记录
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `comment_id`: 评论ID
    /// 
    /// # 返回
    /// 返回点赞记录，未点赞返回None
    pub async fn find_by_user_and_comment(
        db: &DatabaseConnection,
        user_id: i64,
        comment_id: i64,
    ) -> Result<Option<comment_like::Model>, AppError> {
        comment_like::Entity::find()
            .filter(comment_like::Column::UserId.eq(user_id))
            .filter(comment_like::Column::CommentId.eq(comment_id))
            .one(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 批量查询用户对多条评论的点赞状态
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `comment_ids`: 评论ID列表
    /// 
    /// # 返回
    /// 返回用户已点赞的评论ID集合
    pub async fn find_by_user_and_comments(
        db: &DatabaseConnection,
        user_id: i64,
        comment_ids: Vec<i64>,
    ) -> Result<HashSet<i64>, AppError> {
        if comment_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let likes = comment_like::Entity::find()
            .filter(comment_like::Column::UserId.eq(user_id))
            .filter(comment_like::Column::CommentId.is_in(comment_ids))
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        Ok(likes.into_iter().map(|l| l.comment_id).collect())
    }

    /// 创建点赞记录
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `comment_id`: 评论ID
    /// - `user_id`: 用户ID
    /// 
    /// # 返回
    /// 返回创建的点赞记录模型
    pub async fn insert<C: ConnectionTrait>(
        db: &C,
        comment_id: i64,
        user_id: i64,
    ) -> Result<comment_like::Model, AppError> {
        let now = Utc::now();
        let like = comment_like::ActiveModel {
            comment_id: Set(comment_id),
            user_id: Set(user_id),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        like.insert(db).await.map_internal_err("点赞失败")
    }

    /// 根据ID删除点赞记录
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 点赞记录ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        comment_like::Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_internal_err("取消点赞失败")?;
        Ok(())
    }

    /// 删除评论的所有点赞记录
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `comment_id`: 评论ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_comment_id<C: ConnectionTrait>(db: &C, comment_id: i64) -> Result<(), AppError> {
        comment_like::Entity::delete_many()
            .filter(comment_like::Column::CommentId.eq(comment_id))
            .exec(db)
            .await
            .map_internal_err("删除点赞失败")?;
        Ok(())
    }

    /// 批量删除多条评论的所有点赞记录
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `comment_ids`: 评论ID列表
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_comment_ids<C: ConnectionTrait>(db: &C, comment_ids: Vec<i64>) -> Result<(), AppError> {
        if comment_ids.is_empty() {
            return Ok(());
        }
        comment_like::Entity::delete_many()
            .filter(comment_like::Column::CommentId.is_in(comment_ids))
            .exec(db)
            .await
            .map_internal_err("删除点赞失败")?;
        Ok(())
    }
}
