// ============================================================
// CollectionId
// ============================================================

crate::id_type!(CollectionId, "media/");

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

    /// 收藏夹里面没有媒体时, cover即为空
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "media_collection")]
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

        /// 收藏的媒体总数
        pub media_count: i64,

        /// 封面媒体的文件ID
        pub cover_file_id: Option<String>,

        /// 封面媒体的ID
        pub cover_media_id: Option<MediaId>,

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
        pub media_count: u64,
        pub cover_file_id: Option<String>,
        pub cover_media_id: Option<MediaId>,
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
                media_count: model.media_count as u64,
                cover_file_id: model.cover_file_id,
                cover_media_id: model.cover_media_id,
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
