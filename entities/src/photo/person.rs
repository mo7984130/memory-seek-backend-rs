use common::{error::AppError, ext::ResultErrExt};
use insight_face_rs::types::FaceEmbedding;
use sea_orm::entity::prelude::*;

use serde::{Deserialize, Serialize};

use crate::photo::face::FaceId;
use insight_face_rs::PgVector;

// 记得要同步修改model的table_name
pub const TABLE_NAME: &'static str = "photo_person";

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_person")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub name_initials: Option<String>,
    pub cover_face_id: i64,
    pub centroid: PgVector,
    pub face_count: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

pub struct PersonId(pub i64);

pub struct PersonRecord {
    pub id: PersonId,
    pub name: String,
    pub name_initials: Option<String>,
    pub cover_face_id: FaceId,
    pub centroid: FaceEmbedding,
    pub face_count: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl TryFrom<Model> for PersonRecord {
    type Error = AppError;

    fn try_from(value: Model) -> Result<Self, Self::Error> {
        let embedding: FaceEmbedding = value
            .centroid
            .try_into()
            .trace_internal_err("db:photo:person:embedding_from:err", "Embedding转换错误")?;

        Ok(Self {
            id: PersonId(value.id),
            name: value.name,
            name_initials: value.name_initials,
            cover_face_id: FaceId(value.cover_face_id),
            centroid: embedding,
            face_count: value.face_count,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}
