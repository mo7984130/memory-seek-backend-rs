// ============================================================
// MediaLikeId
// ============================================================

crate::id_type!(MediaLikeId, "media/");

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
    use crate::media::media::MediaId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "media_media_like")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: MediaLikeId,

        /// 喜欢的照片
        /// media_id 与 user_id 组成复合唯一键
        ///     一个照片只能被一个用户喜欢一次
        ///     用于判断用户是否喜欢这个照片
        #[sea_orm(unique_key = "media_like")]
        pub media_id: MediaId,

        /// 喜欢者
        #[sea_orm(unique_key = "media_like")]
        pub user_id: UserId,

        /// 创建时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,
    }

    /// 照片点赞记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct MediaLikeRecord {
        pub id: MediaLikeId,
        pub media_id: MediaId,
        pub user_id: UserId,
        pub created_at: DateTime,
    }

    impl From<Model> for MediaLikeRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                media_id: model.media_id,
                user_id: model.user_id,
                created_at: model.created_at,
            }
        }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;
