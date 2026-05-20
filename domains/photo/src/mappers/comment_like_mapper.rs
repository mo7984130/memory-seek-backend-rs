use std::collections::HashSet;

use common::error::AppError;
use common::utils::ResultExt;
use entities::comment_like::*;
use sea_orm::ActiveValue::Set;
use sea_orm::ConnectionTrait;
use sea_orm::QuerySelect;
use sea_orm::prelude::*;

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
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_by_user_and_comment(
        db: &impl ConnectionTrait,
        user_id: i64,
        comment_id: i64,
    ) -> Result<Option<Model>, AppError> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::CommentId.eq(comment_id))
            .one(db)
            .await
            .trace_to_internal_err("db_query_err", "查询失败")
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
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_by_user_and_comments(
        db: &DatabaseConnection,
        user_id: i64,
        comment_ids: Vec<i64>,
    ) -> Result<HashSet<i64>, AppError> {
        if comment_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let likes: HashSet<i64> = Entity::find()
            .select_only()
            .column(Column::CommentId)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::CommentId.is_in(comment_ids))
            .into_values::<i64, Column>()
            .all(db)
            .await
            .trace_to_internal_err("db_query_err", "查询失败")?
            .into_iter()
            .collect();

        Ok(likes)
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
    ///
    /// # 错误
    /// - `AppError`: 数据库插入失败
    pub async fn insert<C: ConnectionTrait>(
        db: &C,
        comment_id: i64,
        user_id: i64,
    ) -> Result<Model, AppError> {
        ActiveModel {
            comment_id: Set(comment_id),
            user_id: Set(user_id),
            ..Default::default()
        }
        .insert(db)
        .await
        .trace_to_internal_err("db_insert_err", "点赞失败")
    }

    /// 根据ID删除点赞记录
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 点赞记录ID
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        Entity::delete_by_id(id)
            .exec(db)
            .await
            .trace_to_internal_err("db_delete_err", "取消点赞失败")?;
        Ok(())
    }

    /// 删除评论的所有点赞记录
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `comment_id`: 评论ID
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败
    pub async fn delete_by_comment_id<C: ConnectionTrait>(
        db: &C,
        comment_id: i64,
    ) -> Result<(), AppError> {
        Entity::delete_many()
            .filter(Column::CommentId.eq(comment_id))
            .exec(db)
            .await
            .trace_to_internal_err("db_delete_err", "删除点赞失败")?;
        Ok(())
    }

    /// 批量删除多条评论的所有点赞记录
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `comment_ids`: 评论ID列表
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败
    pub async fn delete_by_comment_ids(
        db: &impl ConnectionTrait,
        comment_ids: &[i64],
    ) -> Result<(), AppError> {
        if comment_ids.is_empty() {
            return Ok(());
        }
        Entity::delete_many()
            .filter(Column::CommentId.is_in(comment_ids.iter().copied()))
            .exec(db)
            .await
            .trace_to_internal_err("db_delete_err", "删除点赞失败")?;
        Ok(())
    }
}
