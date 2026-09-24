// ============================================================
// CollectionId
// ============================================================

pub use types_core::CollectionId;

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
    use crate::visual::visual::VisualId;

    /// 收藏夹里面没有影像时, cover即为空
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "visual_collection")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: CollectionId,

        /// 所有者
        /// 索引用在 查询用户的收藏夹时
        #[sea_orm(indexed)]
        pub user_id: UserId,

        /// 名称
        #[sea_orm(column_type = "String(StringLen::N(255))")]
        pub name: String,

        /// 描述
        #[sea_orm(column_type = "String(StringLen::N(255))")]
        pub description: Option<String>,

        /// 收藏的影像总数
        pub visual_count: i64,

        /// 封面影像的文件ID
        pub cover_file_id: Option<String>,

        /// 封面影像的ID
        pub cover_visual_id: Option<VisualId>,

        /// 创建时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,

        /// 更新时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub updated_at: DateTime,
    }

    /// 收藏夹记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CollectionRecord {
        pub id: CollectionId,
        pub user_id: UserId,
        pub name: String,
        pub description: Option<String>,
        pub visual_count: u64,
        pub cover_file_id: Option<String>,
        pub cover_visual_id: Option<VisualId>,
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
                visual_count: model.visual_count as u64,
                cover_file_id: model.cover_file_id,
                cover_visual_id: model.cover_visual_id,
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
