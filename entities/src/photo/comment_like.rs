use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

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
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::comment::Entity",
        from = "Column::CommentId",
        to = "super::comment::Column::Id"
    )]
    Comment,
}

impl Related<super::comment::Entity> for Entity {
    /// 返回 CommentLike 到 Comment 的多对一关系定义
    ///
    /// # 返回
    /// `Relation::Comment` 的关系定义
    fn to() -> RelationDef {
        Relation::Comment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
