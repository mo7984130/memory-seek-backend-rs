use common::ext::OkExt;
use common::models::CursorPage;
use common::{
    DbConn as ConnectionTrait,
    error::{AppError, ContextualError, contextual::Result},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, sea_query::Expr,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{comment::*, photo::PhotoId},
};

pub struct CommentMapper;

// 创建
impl CommentMapper {
    /// 插入评论记录.
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
    /// 检查评论记录是否存在.
    pub async fn exists(db: &impl ConnectionTrait, comment_id: CommentId) -> Result<bool> {
        let count = Entity::find()
            .filter(Column::Id.eq(comment_id))
            .count(db)
            .await?;
        Ok(count > 0)
    }

    /// 确保评论存在.
    pub async fn ensure_exist(db: &impl ConnectionTrait, comment_id: CommentId) -> Result<()> {
        if !Self::exists(db, comment_id).await? {
            return Err(ContextualError::warn_without_source(
                "comment_not_exist",
                "评论不存在",
                AppError::not_found("评论不存在"),
            ));
        }
        Ok(())
    }

    /// 增量更新评论点赞计数.
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
    /// 查询照片的热门评论.
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

    /// 分页查询照片评论.
    pub async fn query_by_photo_id(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        exclude_ids: &[CommentId],
        cursor: Option<&TimeIdCursor<CommentId>>,
        size: u64,
    ) -> Result<CursorPage<CommentRecord, ()>> {
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

        let records = query
            .all(db)
            .await?
            .into_iter()
            .map(CommentRecord::from)
            .collect::<Vec<_>>();

        Ok(CursorPage::from_oversize(records, size))
    }
}

// 删除
impl CommentMapper {
    /// 删除单条评论.
    /// 删除成功: 返回评论记录
    /// 删除失败: 返回None
    pub async fn delete(
        db: &impl ConnectionTrait,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<Option<CommentRecord>> {
        Entity::delete_by_id(comment_id)
            .filter(Column::UserId.eq(user_id))
            .exec_with_returning(db)
            .await?
            .pop()
            .map(CommentRecord::from)
            .to_ok()
    }

    /// 删除指定照片的全部评论并返回评论 ID.
    pub async fn delete_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<Vec<CommentId>> {
        // 先查询要删除的评论 ID
        let comment_ids = Entity::delete_many()
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .exec_with_returning(db)
            .await?
            .into_iter()
            .map(|model| model.id)
            .collect();

        Ok(comment_ids)
    }
}
