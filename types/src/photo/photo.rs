// ============================================================
// PhotoId
// ============================================================

crate::id_type!(PhotoId, "photo/");

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common::{
        ContextualResult, DbConn,
        time::{DateTime, now},
    };
    use sea_orm::entity::prelude::*;
    use sea_orm::{ActiveValue::Set, sea_query::Index};
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::auth::user::UserId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_photo")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: PhotoId,

        /// 上传者ID
        pub user_id: UserId,

        /// 名称
        pub name: String,

        /// 文件大小(字节)
        pub size: u64,

        /// 照片宽度(像素)
        pub width: u32,

        /// 照片高度(像素)
        pub height: u32,

        /// 文件 MIME 类型
        pub mime_type: String,

        /// 文件MD4哈希值
        #[sea_orm(unique)]
        pub md5: String,

        /// 存储的文件ID
        #[sea_orm(unique)]
        pub file_id: String,

        /// 喜欢总数
        #[sea_orm(default_value = 0)]
        pub like_count: u64,

        /// 评论总数
        #[sea_orm(default_value = 0)]
        pub comment_count: u64,

        /// 更新时间
        pub updated_at: DateTime,

        /// 创建时间
        pub created_at: DateTime,
    }

    #[common::register_async(
        slice = crate::db_init::INIT_INDEXES,
        ty = crate::db_init::InitIndexFn
    )]
    async fn init_index(db: &DatabaseConnection) -> ContextualResult<()> {
        let stmt = Index::create()
            .name("idx_photo_id_created_at")
            .table(Entity)
            .col(Column::CreatedAt)
            .col(Column::Id)
            .if_not_exists()
            .to_owned();

        db.execute_raw(db.get_database_backend().build(&stmt))
            .await?;

        Ok(())
    }

    /// 照片记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct PhotoRecord {
        pub id: PhotoId,
        pub user_id: UserId,
        pub name: String,
        pub size: u64,
        pub width: u32,
        pub height: u32,
        pub mime_type: String,
        pub md5: String,
        pub file_id: String,
        pub comment_count: u64,
        pub like_count: u64,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    pub struct NewPhotoRecord {
        pub user_id: UserId,
        pub name: String,
        pub size: u64,
        pub width: u32,
        pub height: u32,
        pub mime_type: String,
        pub md5: String,
        pub file_id: String,
    }

    impl From<Model> for PhotoRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                user_id: model.user_id,
                name: model.name,
                size: model.size,
                width: model.width,
                height: model.height,
                mime_type: model.mime_type,
                md5: model.md5,
                file_id: model.file_id,
                comment_count: model.comment_count as u64,
                like_count: model.like_count as u64,
                created_at: model.created_at,
                updated_at: model.updated_at,
            }
        }
    }

    impl From<NewPhotoRecord> for ActiveModel {
        fn from(record: NewPhotoRecord) -> Self {
            Self {
                user_id: Set(record.user_id),
                name: Set(record.name),
                size: Set(record.size),
                width: Set(record.width),
                height: Set(record.height),
                mime_type: Set(record.mime_type),
                md5: Set(record.md5),
                file_id: Set(record.file_id),
                created_at: Set(now()),
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
