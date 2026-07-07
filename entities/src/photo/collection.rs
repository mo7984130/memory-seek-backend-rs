use std::str::FromStr;

use common::error::AppError;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::auth::user::UserId;

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
pub struct CollectionId(pub i64);
impl From<i64> for CollectionId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}
impl FromStr for CollectionId {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<i64>()
            .map(CollectionId)
            .map_err(|_| AppError::BadRequest("无效的 collection_id".into()))
    }
}

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
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

/// 收藏夹记录，使用强类型 ID
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollectionRecord {
    pub id: CollectionId,
    pub user_id: UserId,
    pub name: String,
    pub description: Option<String>,
    pub photo_count: i64,
    pub cover_file_id: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl From<Model> for CollectionRecord {
    fn from(model: Model) -> Self {
        Self {
            id: CollectionId(model.id),
            user_id: UserId(model.user_id),
            name: model.name,
            description: model.description,
            photo_count: model.photo_count,
            cover_file_id: model.cover_file_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
