use common::error::{AppError, DeferredError, DeferredResult as Result};
use common::ext::{DeferOptionExt, DeferResultExt, OkExt};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, sea_query::Expr,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{comment::*, photo::PhotoId},
};

pub struct CommentMapper;

// 创建
impl CommentMapper {
    pub async fn insert(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        user_id: UserId,
        content: String,
    ) -> Result<CommentRecord> {
        ActiveModel {
            photo_id: Set(photo_id),
            user_id: Set(user_id),
            content: Set(content),
            ..Default::default()
        }
        .insert(db)
        .await
        .map(CommentRecord::from)?
        .to_ok()
    }
}

// 修改
impl CommentMapper {
    pub async fn exists(db: &impl ConnectionTrait, comment_id: CommentId) -> Result<bool> {
        let count = Entity::find()
            .filter(Column::Id.eq(comment_id))
            .count(db)
            .await
            .defer_error(
                "db_query_err",
                "查询评论是否存在失败",
                AppError::InternalServerError,
            )?;
        Ok(count > 0)
    }

    pub async fn ensure_exist(db: &impl ConnectionTrait, comment_id: CommentId) -> Result<()> {
        if !Self::exists(db, comment_id).await? {
            return Err(DeferredError::warn_without_source(
                "comment_not_exist",
                "评论不存在",
                AppError::not_found("评论不存在"),
            ));
        }
        Ok(())
    }

    pub async fn update_like_count_delta(
        db: &impl ConnectionTrait,
        comment_id: CommentId,
        delta: i64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::LikeCount, Expr::col(Column::LikeCount).add(delta))
            .filter(Column::Id.eq(comment_id))
            .exec(db)
            .await?;

        Ok(())
    }
}

// 查询
impl CommentMapper {
    pub async fn query_hot_comments(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        min_likes: u64,
        size: u64,
    ) -> Result<Vec<CommentRecord>> {
        Entity::find()
            .filter(Column::PhotoId.eq(photo_id))
            .filter(Column::LikeCount.gt(min_likes))
            .order_by_desc(Column::LikeCount)
            .limit(size)
            .all(db)
            .await?
            .into_iter()
            .map(CommentRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    pub async fn query_by_photo_id(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        exclude_ids: &[CommentId],
        cursor: Option<&TimeIdCursor<CommentId>>,
        size: u64,
    ) -> Result<Vec<CommentRecord>> {
        // 分页契约: 查询 size+1 条, 多出的 1 条用于 has_more 判定,
        // 由 service 层用 CursorPage::from_oversize 截断消费
        let mut query = Entity::find()
            .filter(Column::PhotoId.eq(photo_id))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(size + 1);

        if !exclude_ids.is_empty() {
            query = query.filter(Column::Id.is_not_in(exclude_ids.iter().copied()));
        }

        if let Some(c) = cursor {
            query = query.filter(c.before(Column::CreatedAt, Column::Id));
        }

        query
            .all(db)
            .await?
            .into_iter()
            .map(CommentRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    pub async fn query_photo_id_by_id(
        db: &impl ConnectionTrait,
        comment_id: CommentId,
    ) -> Result<PhotoId> {
        Entity::find()
            .filter(Column::Id.eq(comment_id))
            .select_only()
            .column(Column::PhotoId)
            .into_tuple::<PhotoId>()
            .one(db)
            .await?
            .defer_warn_none(
                "comment_not_found",
                "评论不存在",
                AppError::bad_request("评论不存在"),
            )
    }
}

// 删除
impl CommentMapper {
    pub async fn delete(
        db: &impl ConnectionTrait,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<bool> {
        let ret = Entity::delete_by_id(comment_id)
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(ret.rows_affected == 1)
    }

    pub async fn delete_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<Vec<CommentId>> {
        if photo_ids.is_empty() {
            return Ok(vec![]);
        }

        // 先查询要删除的评论 ID
        let comment_ids: Vec<CommentId> = Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .into_tuple::<CommentId>()
            .all(db)
            .await?;

        if !comment_ids.is_empty() {
            Entity::delete_many()
                .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
                .exec(db)
                .await?;
        }

        Ok(comment_ids)
    }
}
