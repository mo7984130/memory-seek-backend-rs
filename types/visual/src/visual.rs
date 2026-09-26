// ============================================================
// VisualId / VisualKind
// ============================================================

pub use types_core::{VisualId, VisualKind};

#[cfg(feature = "orm")]
use sea_orm::entity::prelude::*;

#[cfg(feature = "orm")]
mod entity {
    use common_core::{
        ContextualResult, DbConn,
        time::{DateTime, now},
    };
    use sea_orm::{ActiveValue::Set, sea_query::Index};
    use serde::{Deserialize, Serialize};

    use super::*;
    use types_core::UserId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "visual_visual")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: VisualId,

        /// 上传者ID
        pub user_id: UserId,

        /// 名称
        pub name: String,

        /// 文件大小(字节)
        pub size: i64,

        /// 宽度(像素)
        pub width: i32,

        /// 高度(像素)
        pub height: i32,

        pub kind: VisualKind,

        /// 视频的长度 (照片为0)
        pub duration_ms: i64,

        /// 文件BLAKE3哈希值
        #[sea_orm(unique)]
        pub hash: String,

        /// 存储的文件ID
        #[sea_orm(unique)]
        pub file_id: String,

        /// 喜欢总数
        #[sea_orm(default_value = 0)]
        pub like_count: i64,

        /// 评论总数
        #[sea_orm(default_value = 0)]
        pub comment_count: i64,

        /// 更新时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub updated_at: DateTime,

        /// 创建时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,
    }

    #[common_macros::register_async(
        send,
        slice = types_db_registry::INIT_INDEXES,
        ty = types_db_registry::InitIndexFn
    )]
    async fn init_index(db: &DatabaseConnection) -> ContextualResult<()> {
        let stmt = Index::create()
            .name("idx_visual_id_created_at")
            .table(Entity)
            .col(Column::CreatedAt)
            .col(Column::Id)
            .if_not_exists()
            .to_owned();

        db.execute_raw(db.get_database_backend().build(&stmt))
            .await?;

        Ok(())
    }

    /// 影像记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct VisualRecord {
        pub id: VisualId,
        pub user_id: UserId,
        pub name: String,
        pub size: u64,
        pub width: u32,
        pub height: u32,
        pub kind: VisualKind,
        pub duration_ms: u64,
        pub hash: String,
        pub file_id: String,
        pub comment_count: u64,
        pub like_count: u64,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    pub struct NewVisualRecord {
        pub user_id: UserId,
        pub name: String,
        pub size: u64,
        pub width: u32,
        pub height: u32,
        pub kind: VisualKind,
        pub duration_ms: u64,
        pub hash: String,
        pub file_id: String,
        /// 指定创建时间；为 `None` 时使用当前时间
        pub created_at: Option<DateTime>,
    }

    impl From<Model> for VisualRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                user_id: model.user_id,
                name: model.name,
                size: model.size as u64,
                width: model.width as u32,
                height: model.height as u32,
                duration_ms: model.duration_ms as u64,
                kind: model.kind,
                hash: model.hash,
                file_id: model.file_id,
                comment_count: model.comment_count as u64,
                like_count: model.like_count as u64,
                created_at: model.created_at,
                updated_at: model.updated_at,
            }
        }
    }

    impl From<NewVisualRecord> for ActiveModel {
        fn from(record: NewVisualRecord) -> Self {
            Self {
                user_id: Set(record.user_id),
                name: Set(record.name),
                size: Set(record.size as i64),
                width: Set(record.width as i32),
                height: Set(record.height as i32),
                duration_ms: Set(record.duration_ms as i64),
                kind: Set(record.kind),
                hash: Set(record.hash),
                file_id: Set(record.file_id),
                created_at: Set(record.created_at.unwrap_or_else(now)),
                updated_at: Set(now()),
                ..Default::default()
            }
        }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;
