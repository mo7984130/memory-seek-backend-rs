use common::{Result, ext::ResultErrExt};
use entities::{
    auth::user::UserId,
    photo::{comment::*, photo::PhotoId},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, entity::prelude::DateTimeUtc,
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
            photo_id: Set(photo_id.0),
            user_id: Set(user_id.0),
            content: Set(content),
            ..Default::default()
        }
        .insert(db)
        .await
        .trace_internal_err("db_insert_err", "插入评论失败")
        .map(CommentRecord::from)
    }
}

// 修改
impl CommentMapper {}

// 查询
impl CommentMapper {
    pub async fn query_hot_comments(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        min_likes: u64,
        size: u64,
    ) -> Result<Vec<CommentRecord>> {
        Entity::find()
            .filter(Column::PhotoId.eq(photo_id.0))
            .filter(Column::LikeCount.gt(min_likes))
            .order_by_desc(Column::LikeCount)
            .limit(size)
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
            .map(|models| models.into_iter().map(CommentRecord::from).collect())
    }

    pub async fn query_by_photo_id(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        exclude_ids: Vec<CommentId>,
        cursor: Option<DateTimeUtc>,
        size: u64,
    ) -> Result<Vec<CommentRecord>> {
        let mut query = Entity::find()
            .filter(Column::PhotoId.eq(photo_id.0))
            .order_by_desc(Column::CreatedAt)
            .limit(size);

        if !exclude_ids.is_empty() {
            query = query.filter(Column::Id.is_not_in(exclude_ids.iter().map(|id| id.0)));
        }

        if let Some(c) = cursor {
            query = query.filter(Column::CreatedAt.lt(c));
        }

        query
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
            .map(|models| models.into_iter().map(CommentRecord::from).collect())
    }
}

// 删除
impl CommentMapper {}
