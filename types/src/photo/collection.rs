use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

use crate::error::ParseIdError;

// ============================================================
// CollectionId
// ============================================================

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct CollectionId(pub i64);

impl From<i64> for CollectionId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<CollectionId> for i64 {
    fn from(id: CollectionId) -> Self {
        id.0
    }
}

impl fmt::Display for CollectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for CollectionId {
    type Err = ParseIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<i64>()
            .map(CollectionId)
            .map_err(|_| ParseIdError("无效 collection_id"))
    }
}

// ============================================================
// SeaORM 支持（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
impl From<CollectionId> for sea_orm::Value {
    fn from(val: CollectionId) -> Self {
        sea_orm::Value::BigInt(Some(val.0))
    }
}

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::auth::user::UserId;

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
        pub cover_photo_id: Option<i64>,
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
        pub cover_photo_id: Option<i64>,
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
                cover_photo_id: model.cover_photo_id,
                created_at: model.created_at,
                updated_at: model.updated_at,
            }
        }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;
