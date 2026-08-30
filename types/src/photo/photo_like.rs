// ============================================================
// PhotoLikeId
// ============================================================

crate::id_type!(PhotoLikeId, "photo/");

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
    use crate::photo::photo::PhotoId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_photo_like")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: PhotoLikeId,

        /// 喜欢的照片
        pub photo_id: PhotoId,

        /// 喜欢者
        pub user_id: UserId,

        /// 创建时间
        pub created_at: DateTime,
    }

    /// 创建索引
    /// PhotoId 和 UserId 复合唯一索引
    ///     一个照片只能被一个用户喜欢一次
    ///     用于判断用户是否喜欢这个照片
    #[common::register_async(
        slice = crate::db_init::INIT_INDEXES,
        ty = crate::db_init::InitIndexFn
    )]
    async fn init_index(db: &DatabaseConnection) -> ContextualResult<()> {
        let stmt = Index::create()
            .name("idx_photo_like_photo_id_user_id")
            .table(Entity)
            .col(Column::PhotoId)
            .col(Column::UserId)
            .if_not_exists()
            .unique()
            .to_owned();
        db.execute_raw(db.get_database_backend().build(&stmt))
            .await?;

        Ok(())
    }

    /// 照片点赞记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct PhotoLikeRecord {
        pub id: PhotoLikeId,
        pub photo_id: PhotoId,
        pub user_id: UserId,
        pub created_at: DateTime,
    }

    impl From<Model> for PhotoLikeRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
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
