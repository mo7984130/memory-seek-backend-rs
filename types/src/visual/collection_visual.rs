// ============================================================
// CollectionVisualId
// ============================================================

pub use types_core::CollectionVisualId;

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common::ContextualResult;
    use common::time::DateTime;
    use sea_orm::entity::prelude::*;
    use sea_orm::sea_query::Index;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::visual::collection::CollectionId;
    use crate::visual::visual::VisualId;
    use types_core::UserId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "visual_collection_visual")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: CollectionVisualId,

        /// 所属收藏夹的ID
        /// collection_id 与 visual_id 组成复合唯一键
        ///     一个影像只能被一个收藏夹收藏一次
        ///     用于判断某个收藏夹中是否存在某个影像
        #[sea_orm(unique_key = "collection_visual")]
        pub collection_id: CollectionId,

        /// 收藏的影像ID
        #[sea_orm(unique_key = "collection_visual")]
        pub visual_id: VisualId,

        /// 收藏夹所有者
        pub user_id: UserId,

        /// 创建时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,
    }

    /// 创建索引
    /// CollectionId 和 CreatedAt 复合索引, 用于 按照收藏时间获取收藏夹里面影像时
    #[common::register_async(
        send,
        slice = crate::db_init::INIT_INDEXES,
        ty = crate::db_init::InitIndexFn
    )]
    async fn init_index(db: &DatabaseConnection) -> ContextualResult<()> {
        let stmt = Index::create()
            .name("idx_collection_id_created_at")
            .table(Entity)
            .col(Column::CollectionId)
            .col(Column::CreatedAt)
            .if_not_exists()
            .to_owned();
        db.execute_raw(db.get_database_backend().build(&stmt))
            .await?;

        Ok(())
    }

    /// 收藏夹影像记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CollectionVisualRecord {
        pub id: CollectionVisualId,
        pub collection_id: CollectionId,
        pub visual_id: VisualId,
        pub user_id: UserId,
        pub created_at: DateTime,
    }

    impl From<Model> for CollectionVisualRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                collection_id: model.collection_id,
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
