use crate::vector::DrVector;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_face_person")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub name_initials: Option<String>,
    pub max_score_feature_id: i64,
    pub max_score: f32,
    pub total_photo_count: i64,
    pub centroid_embedding: DrVector,
    pub total_weight_count: f32,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::face_feature::Entity")]
    FaceFeatures,
}

impl Related<super::face_feature::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FaceFeatures.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
