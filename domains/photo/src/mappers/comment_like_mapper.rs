use std::collections::HashSet;

use common::Result;
use common::ext::ToOk;
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect};
use types::photo::comment_like::*;
use types::{auth::user::UserId, photo::comment::CommentId};

pub struct CommentLikeMapper;

// 创建
impl CommentLikeMapper {
    pub async fn insert(
        db: &impl ConnectionTrait,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<bool> {
        let now = chrono::Utc::now();

        let active_model = ActiveModel {
            comment_id: Set(comment_id.0),
            user_id: Set(user_id.0),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let result = Entity::insert(active_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([Column::CommentId, Column::UserId])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;

        Ok(result > 0)
    }
}

// 修改
impl CommentLikeMapper {}

// 查询
impl CommentLikeMapper {
    pub async fn query_is_like_by_comment_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        comment_ids: Vec<CommentId>,
    ) -> Result<HashSet<CommentId>> {
        if comment_ids.is_empty() {
            return HashSet::new().to_ok();
        }

        Entity::find()
            .select_only()
            .column(Column::CommentId)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::CommentId.is_in(comment_ids))
            .into_tuple::<i64>()
            .all(db)
            .await?
            .into_iter()
            .map(CommentId)
            .collect::<HashSet<CommentId>>()
            .to_ok()
    }
}

// 删除
impl CommentLikeMapper {
    pub async fn delete(
        db: &impl ConnectionTrait,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<bool> {
        let res = Entity::delete_many()
            .filter(Column::CommentId.eq(comment_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(res.rows_affected != 0)
    }

    pub async fn delete_all_by_comment_id(
        db: &impl ConnectionTrait,
        comment_id: CommentId,
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::CommentId.eq(comment_id))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    pub async fn delete_by_comment_ids(
        db: &impl ConnectionTrait,
        comment_ids: &[CommentId],
    ) -> Result<u64> {
        if comment_ids.is_empty() {
            return Ok(0);
        }

        Entity::delete_many()
            .filter(Column::CommentId.is_in(comment_ids.iter().copied()))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
