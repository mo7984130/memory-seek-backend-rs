use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_face_person")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub max_score_feature_id: i64,
    pub max_score: f32,
    pub total_photo_count: i64,
    #[sea_orm(column_type = "Text")]
    pub centroid_embedding: String,
    pub total_weight_count: f32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
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

impl Model {
    pub fn centroid_to_vec(&self) -> Vec<f32> {
        self.centroid_embedding
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .filter_map(|s| s.trim().parse::<f32>().ok())
            .collect()
    }

    pub fn centroid_from_vec(vec: &[f32]) -> String {
        format!("[{}]", vec.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","))
    }
}
