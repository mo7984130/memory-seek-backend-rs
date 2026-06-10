use std::collections::HashSet;

use common::Result;
use common::ext::{ResultErrExt, ToErr, ToOk, log_err};
use entities::photo::comment_like::*;
use entities::{auth::user::UserId, photo::comment::CommentId};
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect};

pub struct CommentLikeMapper;

// 创建
impl CommentLikeMapper {
    pub async fn insert_ignore(
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
            .exec(db)
            .await;

        match result {
            Ok(_) => Ok(true),
            Err(DbErr::RecordNotInserted) => Ok(false),
            Err(e) => log_err(
                "db_insert_err",
                "插入照片评论喜欢时错误",
                e,
                common::error::AppError::InternalServerError,
            )
            .to_err(),
        }
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
            .filter(Column::UserId.eq(user_id.0))
            .filter(Column::CommentId.is_in(comment_ids.into_iter().map(|id| id.0)))
            .into_values::<i64, Column>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询评论是否喜欢数据库错误")?
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
        let res = Entity::delete_by_id(comment_id.0)
            .filter(Column::UserId.eq(user_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "尝试删除照片评论喜欢错误")?;

        return Ok(res.rows_affected != 0);
    }

    pub async fn delete_all_by_comment_id(
        db: &impl ConnectionTrait,
        comment_id: CommentId,
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::CommentId.eq(comment_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_del_err", "根据评论id删除所有评论喜欢数据库错误")?
            .rows_affected
            .to_ok()
    }
}
