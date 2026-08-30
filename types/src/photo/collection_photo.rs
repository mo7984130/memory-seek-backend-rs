// ============================================================
// CollectionPhotoId
// ============================================================

crate::id_type!(CollectionPhotoId, "photo/");

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
    use crate::auth::user::UserId;
    use crate::photo::collection::CollectionId;
    use crate::photo::photo::PhotoId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_collection_photo")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: CollectionPhotoId,

        /// 所属收藏夹的ID
        pub collection_id: CollectionId,

        /// 收藏的照片ID
        pub photo_id: PhotoId,

        /// 收藏夹所有者
        pub user_id: UserId,

        /// 创建时间
        pub created_at: DateTime,
    }

    /// 创建索引
    /// CollectionId 和 PhotoId 复合唯一索引
    ///     一个照片只能被一个收藏夹收藏一次
    ///     用于判断某个收藏夹中是否存在某个照片
    /// CollectionId 和 CreatedAt 复合索引, 用于 按照收藏时间获取收藏夹里面照片时
    #[common::register_async(
        slice = crate::db_init::INIT_INDEXES,
        ty = crate::db_init::InitIndexFn
    )]
    async fn init_index(db: &DatabaseConnection) -> ContextualResult<()> {
        let stmt = Index::create()
            .name("idx_collection_id_photo_id")
            .table(Entity)
            .col(Column::CollectionId)
            .col(Column::PhotoId)
            .if_not_exists()
            .unique()
            .to_owned();
        db.execute_raw(db.get_database_backend().build(&stmt))
            .await?;

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

    /// 收藏夹照片记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CollectionPhotoRecord {
        pub id: CollectionPhotoId,
        pub collection_id: CollectionId,
        pub photo_id: PhotoId,
        pub user_id: UserId,
        pub created_at: DateTime,
    }

    impl From<Model> for CollectionPhotoRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                collection_id: model.collection_id,
                photo_id: model.photo_id,
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
