use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_collection")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub photo_count: i64,
    pub cover_file_id: Option<String>,
    pub is_favorite: bool,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::collection_photo::Entity")]
    CollectionPhotos,
}

impl Related<super::collection_photo::Entity> for Entity {
    /// 返回 Collection 到 CollectionPhoto 的一对多关系定义
    ///
    /// # 返回
    /// `Relation::CollectionPhotos` 的关系定义
    fn to() -> RelationDef {
        Relation::CollectionPhotos.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
