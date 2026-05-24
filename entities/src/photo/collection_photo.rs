use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub struct CollectionPhotoId(pub i64);

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
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
