use std::fmt;

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

// ============================================================
// CommentId
// ============================================================

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct CommentId(pub i64);

impl From<i64> for CommentId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<CommentId> for i64 {
    fn from(id: CommentId) -> Self {
        id.0
    }
}

impl fmt::Display for CommentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================
// SeaORM 支持（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
impl From<CommentId> for sea_orm::Value {
    fn from(val: CommentId) -> Self {
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
    use crate::photo::photo::PhotoId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_comment")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub photo_id: i64,
        pub user_id: i64,
        pub content: String,
        pub like_count: i32,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    /// 评论记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CommentRecord {
        pub id: CommentId,
        pub photo_id: PhotoId,
        pub user_id: UserId,
        pub content: String,
        pub like_count: i32,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    impl From<Model> for CommentRecord {
        fn from(model: Model) -> Self {
            Self {
                id: CommentId(model.id),
                photo_id: PhotoId(model.photo_id),
                user_id: UserId(model.user_id),
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
