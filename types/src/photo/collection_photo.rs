use std::fmt;

use serde::{Deserialize, Serialize};

// ============================================================
// CollectionPhotoId
// ============================================================

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
pub struct CollectionPhotoId(pub i64);

impl From<i64> for CollectionPhotoId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<CollectionPhotoId> for i64 {
    fn from(id: CollectionPhotoId) -> Self {
        id.0
    }
}

impl fmt::Display for CollectionPhotoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================
// SeaORM 支持（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
impl From<CollectionPhotoId> for sea_orm::Value {
    fn from(val: CollectionPhotoId) -> Self {
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
    use crate::photo::collection::CollectionId;
    use crate::photo::photo::PhotoId;

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

    /// 收藏夹照片记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CollectionPhotoRecord {
        pub id: CollectionPhotoId,
        pub collection_id: CollectionId,
        pub photo_id: PhotoId,
        pub user_id: UserId,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    impl From<Model> for CollectionPhotoRecord {
        fn from(model: Model) -> Self {
            Self {
                id: CollectionPhotoId(model.id),
                collection_id: CollectionId(model.collection_id),
                photo_id: PhotoId(model.photo_id),
                user_id: UserId(model.user_id),
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
