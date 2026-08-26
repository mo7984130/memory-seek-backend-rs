// ============================================================
// PhotoId
// ============================================================

crate::id_type!(PhotoId, "photo/");

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common::time::{now, DateTime};
    use sea_orm::entity::prelude::*;
    use sea_orm::ActiveValue::Set;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::auth::user::UserId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_photo")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: PhotoId,
        pub user_id: UserId,
        pub name: String,
        pub size: i64,
        pub width: i32,
        pub height: i32,
        pub mime_type: String,
        pub md5: String,
        pub file_id: String,
        pub comment_count: i64,
        pub like_count: i64,
        pub created_at: DateTime,
        pub updated_at: DateTime,
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
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    pub struct NewPhotoRecord {
        pub user_id: UserId,
        pub name: String,
        pub size: i64,
        pub width: i32,
        pub height: i32,
        pub mime_type: String,
        pub md5: String,
        pub file_id: String,
    }

    impl From<Model> for PhotoRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                user_id: model.user_id,
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

    impl From<NewPhotoRecord> for ActiveModel {
        fn from(record: NewPhotoRecord) -> Self {
            Self {
                user_id: Set(record.user_id),
                name: Set(record.name),
                size: Set(record.size),
                width: Set(record.width),
                height: Set(record.height),
                mime_type: Set(record.mime_type),
                md5: Set(record.md5),
                file_id: Set(record.file_id),
                created_at: Set(now()),
                updated_at: Set(now()),
                ..Default::default()
            }
        }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;
