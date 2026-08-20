// ============================================================
// CommentLikeId
// ============================================================

crate::id_type!(CommentLikeId, "photo/");

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use super::*;
    use crate::auth::user::UserId;
    use crate::photo::comment::CommentId;
    use common::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_comment_like")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: CommentLikeId,
        pub comment_id: CommentId,
        pub user_id: UserId,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    /// 评论点赞记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CommentLikeRecord {
        pub id: CommentLikeId,
        pub comment_id: CommentId,
        pub user_id: UserId,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    impl From<Model> for CommentLikeRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                comment_id: model.comment_id,
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
