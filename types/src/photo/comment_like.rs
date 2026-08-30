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
    use common::{ContextualResult, time::DateTime};
    use sea_orm::{entity::prelude::*, sea_query::Index};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_comment_like")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: CommentLikeId,

        /// 评论ID
        pub comment_id: CommentId,

        /// 喜欢者ID
        pub user_id: UserId,

        // 创建时间
        pub created_at: DateTime,
    }

    /// 创建索引
    /// CommentId 和 UserId 复合唯一索引
    ///     一个评论只能被一个用户喜欢一次
    ///     用于判断用户是否喜欢这个评论
    #[common::register_async(
        slice = crate::db_init::INIT_INDEXES,
        ty = crate::db_init::InitIndexFn
    )]
    async fn init_index(db: &DatabaseConnection) -> ContextualResult<()> {
        let stmt = Index::create()
            .name("idx_comment_like_comment_id_user_id")
            .table(Entity)
            .col(Column::CommentId)
            .col(Column::UserId)
            .if_not_exists()
            .unique()
            .to_owned();
        db.execute_raw(db.get_database_backend().build(&stmt))
            .await?;

        Ok(())
    }

    /// 评论点赞记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CommentLikeRecord {
        pub id: CommentLikeId,
        pub comment_id: CommentId,
        pub user_id: UserId,
        pub created_at: DateTime,
    }

    impl From<Model> for CommentLikeRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                comment_id: model.comment_id,
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
