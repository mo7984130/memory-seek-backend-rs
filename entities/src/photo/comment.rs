use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize)]
pub struct CommentId(pub i64);

impl From<i64> for CommentId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_comment")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub photo_id: i64,
    pub user_id: i64,
    pub content: String,
    pub like_count: i32,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

/// 评论记录，使用强类型 ID
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommentRecord {
    pub id: CommentId,
    pub photo_id: i64,
    pub user_id: i64,
    pub content: String,
    pub like_count: i32,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl From<Model> for CommentRecord {
    fn from(model: Model) -> Self {
        Self {
            id: CommentId(model.id),
            photo_id: model.photo_id,
            user_id: model.user_id,
            content: model.content,
            like_count: model.like_count,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
