use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_photo")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub size: i64,
    pub width: i32,
    pub height: i32,
    pub mime_type: String,
    pub md5: String,
    pub file_id: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::face_feature::Entity")]
    FaceFeatures,
    #[sea_orm(has_many = "super::collection_photo::Entity")]
    CollectionPhotos,
    #[sea_orm(has_many = "super::comment::Entity")]
    Comments,
}

impl Related<super::face_feature::Entity> for Entity {
    /// 返回 Photo 到 FaceFeature 的一对多关系定义
    ///
    /// # 返回
    /// `Relation::FaceFeatures` 的关系定义
    fn to() -> RelationDef {
        Relation::FaceFeatures.def()
    }
}

impl Related<super::collection_photo::Entity> for Entity {
    /// 返回 Photo 到 CollectionPhoto 的一对多关系定义
    ///
    /// # 返回
    /// `Relation::CollectionPhotos` 的关系定义
    fn to() -> RelationDef {
        Relation::CollectionPhotos.def()
    }
}

impl Related<super::comment::Entity> for Entity {
    /// 返回 Photo 到 Comment 的一对多关系定义
    ///
    /// # 返回
    /// `Relation::Comments` 的关系定义
    fn to() -> RelationDef {
        Relation::Comments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
