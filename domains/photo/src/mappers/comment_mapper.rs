use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use entities::comment::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, sea_query::Expr,
};

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
    pub async fn query_by_id(db: &DatabaseConnection, id: i64) -> Result<Model, AppError> {
        Entity::find_by_id(id)
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")?
            .ok_or_else(|| AppError::not_found("评论不存在"))
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
    pub async fn query_hot_comments(
        db: &DatabaseConnection,
        photo_id: i64,
        min_likes: i32,
        limit: u64,
    ) -> Result<Vec<Model>, AppError> {
        Entity::find()
            .filter(Column::PhotoId.eq(photo_id))
            .filter(Column::LikeCount.gt(min_likes))
            .order_by_desc(Column::LikeCount)
            .limit(limit)
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
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
    pub async fn query_by_photo_id(
        db: &DatabaseConnection,
        photo_id: i64,
        exclude_ids: &[i64],
        cursor: Option<DateTime<Utc>>,
        limit: u64,
    ) -> Result<Vec<Model>, AppError> {
        let l = limit + 1;
        let mut query = Entity::find()
            .filter(Column::PhotoId.eq(photo_id))
            .order_by_desc(Column::CreatedAt)
            .limit(l);

        if !exclude_ids.is_empty() {
            query = query.filter(Column::Id.is_not_in(exclude_ids.iter().copied()));
        }

        if let Some(c) = cursor {
            query = query.filter(Column::CreatedAt.lt(c));
        }

        query
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
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
    ) -> Result<Model, AppError> {
        ActiveModel {
            photo_id: Set(photo_id),
            user_id: Set(user_id),
            content: Set(content),
            ..Default::default()
        }
        .insert(db)
        .await
        .trace_internal_err("db_insert_err", "发布失败")
    }

    /// 删除评论
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 评论ID
    ///
    /// # 返回
    /// 成功返回空元组
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败时返回错误
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        Entity::delete_by_id(id)
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "删除失败")?;
        Ok(())
    }

    /// 更新评论点赞数
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 评论ID
    /// - `delta`: 点赞数变化量（正数增加，负数减少）
    ///
    /// # 错误
    /// - `AppError`: 数据库更新失败时返回错误
    pub async fn update_like_count<C: ConnectionTrait>(
        db: &C,
        id: i64,
        delta: i32,
    ) -> Result<(), AppError> {
        Entity::update_many()
            .col_expr(
                Column::LikeCount,
                Expr::cust_with_values("GREATEST(like_count + $1, 0)", [delta]),
            )
            .col_expr(Column::UpdatedAt, Expr::cust("NOW()"))
            .filter(Column::Id.eq(id))
            .exec(db)
            .await
            .trace_internal_err("db_update_err", "更新失败")?;

        Ok(())
    }

    /// 查询照片的所有评论ID
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 返回评论ID列表
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败时返回错误
    pub async fn query_ids_by_photo_id<C: ConnectionTrait>(
        db: &C,
        photo_id: i64,
    ) -> Result<Vec<i64>, AppError> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::PhotoId.eq(photo_id))
            .into_values::<i64, Column>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
    }

    /// 批量查询多张照片的评论ID列表
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `photo_ids`: 照片ID列表
    ///
    /// # 返回
    /// 返回所有匹配照片的评论ID列表
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败时返回错误
    pub async fn query_ids_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[i64],
    ) -> Result<Vec<i64>, AppError> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .into_values::<i64, Column>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "获取评论id错误")
    }

    /// 根据ID列表批量删除评论
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `ids`: 评论ID列表
    ///
    /// # 返回
    /// 成功返回空元组
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败时返回错误
    pub async fn delete_by_ids(db: &impl ConnectionTrait, ids: &[i64]) -> Result<(), AppError> {
        Entity::delete_many()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .exec(db)
            .await
            .trace_internal_err("db_del_err", "根据评论id删除评论错误")?;

        Ok(())
    }

    /// 删除照片的所有评论
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 成功返回空元组
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败时返回错误
    pub async fn delete_by_photo_id<C: ConnectionTrait>(
        db: &C,
        photo_id: i64,
    ) -> Result<(), AppError> {
        Entity::delete_many()
            .filter(Column::PhotoId.eq(photo_id))
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "删除评论失败")?;
        Ok(())
    }
}
