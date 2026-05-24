use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub struct CommentLikeId(pub i64);

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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
