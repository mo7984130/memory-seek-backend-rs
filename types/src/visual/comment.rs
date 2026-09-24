// ============================================================
// CommentId
// ============================================================

pub use types_core::CommentId;

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
    use crate::visual::visual::VisualId;
    use types_core::UserId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "visual_comment")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: CommentId,

        /// 评论的影像
        pub visual_id: VisualId,

        /// 评论者
        pub user_id: UserId,

        /// 评论内容
        pub content: String,

        /// 喜欢该评论的数量
        #[sea_orm(default_value = 0)]
        pub like_count: i32,

        /// 修改时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub updated_at: DateTime,

        /// 创建时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,
    }

    /// 创建索引
    /// VisualId 和 CreatedAt 复合索引, 用于 查询影像的评论
    /// VisualId 和 LikeCount 复合索引, 用于 查询影像的热门评论
    #[common::register_async(
        send,
        slice = crate::db_init::INIT_INDEXES,
        ty = crate::db_init::InitIndexFn
    )]
    async fn init_index(db: &DatabaseConnection) -> ContextualResult<()> {
        let stmt = Index::create()
            .name("idx_visual_comment_visual_id_created_at")
            .table(Entity)
            .col(Column::VisualId)
            .col(Column::CreatedAt)
            .if_not_exists()
            .to_owned();
        db.execute_raw(db.get_database_backend().build(&stmt))
            .await?;

        let stmt = Index::create()
            .name("idx_visual_comment_visual_id_like_count")
            .table(Entity)
            .col(Column::VisualId)
            .col(Column::LikeCount)
            .if_not_exists()
            .to_owned();
        db.execute_raw(db.get_database_backend().build(&stmt))
            .await?;

        Ok(())
    }

    /// 评论记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CommentRecord {
        pub id: CommentId,
        pub visual_id: VisualId,
        pub user_id: UserId,
        pub content: String,
        pub like_count: u32,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    impl From<Model> for CommentRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                visual_id: model.visual_id,
                user_id: model.user_id,
                content: model.content,
                like_count: model.like_count as u32,
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
