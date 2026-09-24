// ============================================================
// VisualLikeId
// ============================================================

pub use types_core::VisualLikeId;

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common_core::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::visual::VisualId;
    use types_core::UserId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "visual_visual_like")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: VisualLikeId,

        /// 喜欢的影像
        /// visual_id 与 user_id 组成复合唯一键
        ///     一个影像只能被一个用户喜欢一次
        ///     用于判断用户是否喜欢这个影像
        #[sea_orm(unique_key = "visual_like")]
        pub visual_id: VisualId,

        /// 喜欢者
        #[sea_orm(unique_key = "visual_like")]
        pub user_id: UserId,

        /// 创建时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,
    }

    /// 影像点赞记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct VisualLikeRecord {
        pub id: VisualLikeId,
        pub visual_id: VisualId,
        pub user_id: UserId,
        pub created_at: DateTime,
    }

    impl From<Model> for VisualLikeRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                visual_id: model.visual_id,
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
