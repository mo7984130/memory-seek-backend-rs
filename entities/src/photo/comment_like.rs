use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::auth::user::UserId;
use super::comment::CommentId;

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
pub struct CommentLikeId(pub i64);

impl From<i64> for CommentLikeId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_comment_like")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub comment_id: i64,
    pub user_id: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

/// 评论点赞记录，使用强类型 ID
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommentLikeRecord {
    pub id: CommentLikeId,
    pub comment_id: CommentId,
    pub user_id: UserId,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl From<Model> for CommentLikeRecord {
    fn from(model: Model) -> Self {
        Self {
            id: CommentLikeId(model.id),
            comment_id: CommentId(model.comment_id),
            user_id: UserId(model.user_id),
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
