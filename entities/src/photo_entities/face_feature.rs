use crate::vector::PostgreVector;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub const FEATURE_DIM: usize = 512;

/// 512 维人脸特征嵌入向量
pub type Embedding512 = PostgreVector<FEATURE_DIM>;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_face_feature")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub photo_id: i64,
    pub person_id: Option<i64>,
    pub embedding: Embedding512,
    #[sea_orm(column_type = "Json")]
    pub bbox: Json,
    pub score: f32,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::photo::Entity",
        from = "Column::PhotoId",
        to = "super::photo::Column::Id"
    )]
    Photo,
    #[sea_orm(
        belongs_to = "super::face_person::Entity",
        from = "Column::PersonId",
        to = "super::face_person::Column::Id"
    )]
    Person,
}

impl Related<super::photo::Entity> for Entity {
    /// 返回 FaceFeature 到 Photo 的多对一关系定义
    ///
    /// # 返回
    /// `Relation::Photo` 的关系定义
    fn to() -> RelationDef {
        Relation::Photo.def()
    }
}

impl Related<super::face_person::Entity> for Entity {
    /// 返回 FaceFeature 到 FacePerson 的多对一关系定义
    ///
    /// # 返回
    /// `Relation::Person` 的关系定义
    fn to() -> RelationDef {
        Relation::Person.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceBBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
