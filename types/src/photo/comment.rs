// ============================================================
// CommentId
// ============================================================

crate::id_type!(CommentId, "photo/");

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
    #[sea_orm(table_name = "photo_comment")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: CommentId,
        pub photo_id: PhotoId,
        pub user_id: UserId,
        pub content: String,
        pub like_count: i32,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    /// 评论记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CommentRecord {
        pub id: CommentId,
        pub photo_id: PhotoId,
        pub user_id: UserId,
        pub content: String,
        pub like_count: i32,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    impl From<Model> for CommentRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                photo_id: model.photo_id,
                user_id: model.user_id,
                content: model.content,
                like_count: model.like_count,
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
