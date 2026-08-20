// ============================================================
// CollectionId
// ============================================================

crate::id_type!(CollectionId, "photo/");

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::auth::user::UserId;
    use crate::photo::photo::PhotoId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_collection")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: CollectionId,
        pub user_id: UserId,
        pub name: String,
        pub description: Option<String>,
        pub photo_count: i64,
        pub cover_file_id: Option<String>,
        pub cover_photo_id: Option<PhotoId>,
        pub created_at: DateTime,
        pub updated_at: DateTime,
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
        pub cover_photo_id: Option<PhotoId>,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    impl From<Model> for CollectionRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                user_id: model.user_id,
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
