use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_collection_photo")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub collection_id: i64,
    pub photo_id: i64,
    pub user_id: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::collection::Entity",
        from = "Column::CollectionId",
        to = "super::collection::Column::Id"
    )]
    Collection,
    #[sea_orm(
        belongs_to = "super::photo::Entity",
        from = "Column::PhotoId",
        to = "super::photo::Column::Id"
    )]
    Photo,
}

impl Related<super::collection::Entity> for Entity {
    /// 返回 CollectionPhoto 到 Collection 的多对一关系定义
    ///
    /// # 返回
    /// `Relation::Collection` 的关系定义
    fn to() -> RelationDef {
        Relation::Collection.def()
    }
}

impl Related<super::photo::Entity> for Entity {
    /// 返回 CollectionPhoto 到 Photo 的多对一关系定义
    ///
    /// # 返回
    /// `Relation::Photo` 的关系定义
    fn to() -> RelationDef {
        Relation::Photo.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
