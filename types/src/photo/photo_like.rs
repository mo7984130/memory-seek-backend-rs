// ============================================================
// PhotoLikeId
// ============================================================

crate::id_type!(PhotoLikeId, "photo/");

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::auth::user::UserId;
    use crate::photo::photo::PhotoId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_photo_like")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: PhotoLikeId,
        pub photo_id: PhotoId,
        pub user_id: UserId,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    /// 照片点赞记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct PhotoLikeRecord {
        pub id: PhotoLikeId,
        pub photo_id: PhotoId,
        pub user_id: UserId,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    impl From<Model> for PhotoLikeRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                photo_id: model.photo_id,
                user_id: model.user_id,
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
