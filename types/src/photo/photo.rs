// ============================================================
// PhotoId
// ============================================================

crate::id_type!(PhotoId, "photo/");

impl PhotoId {
    pub fn parse_from_str_or_none(s: &str) -> Option<Self> {
        let id = s.parse::<i64>().ok()?;
        Some(Self(id))
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
    #[sea_orm(table_name = "photo_photo")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub user_id: i64,
        pub name: String,
        pub size: i64,
        pub width: i32,
        pub height: i32,
        pub mime_type: String,
        pub md5: String,
        pub file_id: String,
        pub comment_count: i64,
        pub like_count: i64,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    /// 照片记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct PhotoRecord {
        pub id: PhotoId,
        pub user_id: UserId,
        pub name: String,
        pub size: i64,
        pub width: i32,
        pub height: i32,
        pub mime_type: String,
        pub md5: String,
        pub file_id: String,
        pub comment_count: u64,
        pub like_count: u64,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    impl From<Model> for PhotoRecord {
        fn from(model: Model) -> Self {
            Self {
                id: PhotoId(model.id),
                user_id: UserId(model.user_id),
                name: model.name,
                size: model.size,
                width: model.width,
                height: model.height,
                mime_type: model.mime_type,
                md5: model.md5,
                file_id: model.file_id,
                comment_count: model.comment_count as u64,
                like_count: model.like_count as u64,
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
