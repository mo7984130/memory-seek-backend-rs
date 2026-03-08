use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_face_feature")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub photo_id: i64,
    pub person_id: Option<i64>,
    #[sea_orm(column_type = "Text")]
    pub embedding: String,
    #[sea_orm(column_type = "Json")]
    pub bbox: Json,
    pub score: f32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
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
    fn to() -> RelationDef {
        Relation::Photo.def()
    }
}

impl Related<super::face_person::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Person.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn embedding_to_vec(&self) -> Vec<f32> {
        self.embedding
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .filter_map(|s| s.trim().parse::<f32>().ok())
            .collect()
    }

    pub fn embedding_from_vec(vec: &[f32]) -> String {
        format!("[{}]", vec.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceBBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
