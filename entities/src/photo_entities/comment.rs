use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::photo::Entity",
        from = "Column::PhotoId",
        to = "super::photo::Column::Id"
    )]
    Photo,
    #[sea_orm(has_many = "super::comment_like::Entity")]
    Likes,
}

impl Related<super::photo::Entity> for Entity {
    /// 返回 Comment 到 Photo 的多对一关系定义
    ///
    /// # 返回
    /// `Relation::Photo` 的关系定义
    fn to() -> RelationDef {
        Relation::Photo.def()
    }
}

impl Related<super::comment_like::Entity> for Entity {
    /// 返回 Comment 到 CommentLike 的一对多关系定义
    ///
    /// # 返回
    /// `Relation::Likes` 的关系定义
    fn to() -> RelationDef {
        Relation::Likes.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
