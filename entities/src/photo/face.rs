use crate::photo::{person::PersonId, photo::PhotoId};
use common::error::AppError;
use common::ext::ResultErrExt;
use insight_face_rs::PgVector;
use insight_face_rs::types::{BoundingBox, Face, FaceEmbedding, FaceLandmarks};
use sea_orm::{
    ActiveValue::{NotSet, Set},
    entity::prelude::*,
};
use serde::{Deserialize, Serialize};

// 记得要同步修改model的table_name
pub const TABLE_NAME: &'static str = "photo_face";

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_face")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub photo_id: i64,
    pub person_id: Option<i64>,

    #[sea_orm(column_type = "Json")]
    pub bbox: Json,
    #[sea_orm(column_type = "Json")]
    pub landmarks: Json,
    pub score: f32,

    pub embedding: PgVector,

    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

pub struct FaceId(pub i64);

pub struct FaceRecord {
    pub id: FaceId,
    pub photo_id: PhotoId,
    pub person_id: Option<PersonId>,

    pub bbox: BoundingBox,
    pub landmarks: FaceLandmarks,
    pub score: f32,

    pub embedding: FaceEmbedding,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
impl TryFrom<Model> for FaceRecord {
    type Error = AppError;
    fn try_from(value: Model) -> Result<Self, Self::Error> {
        let bbox: BoundingBox = serde_json::from_value(value.bbox)
            .trace_internal_err("db:photo:face:bbox_from:err", "BoundingBox转换错误")?;
        let landmarks: FaceLandmarks = serde_json::from_value(value.landmarks)
            .trace_internal_err("db:photo:face:landmark_from:err", "Landmarks转换错误")?;
        let embedding: FaceEmbedding = value.embedding.into();
        // .trace_internal_err("db:photo:face:embedding_from:err", "Embedding转换错误")?;

        Ok(Self {
            id: FaceId(value.id),
            photo_id: PhotoId(value.photo_id),
            person_id: value.person_id.map(PersonId),
            bbox,
            landmarks,
            score: value.score,
            embedding,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

pub struct NewFaceRecord {
    pub photo_id: PhotoId,
    pub person_id: Option<PersonId>,
    pub bbox: BoundingBox,
    pub landmarks: FaceLandmarks,
    pub score: f32,
    pub embedding: FaceEmbedding,
}
impl NewFaceRecord {
    pub fn from_detected(photo_id: PhotoId, face: Face) -> Self {
        Self {
            photo_id,
            person_id: None,
            bbox: face.bbox,
            landmarks: face.landmarks,
            score: face.score,
            embedding: face.embedding,
        }
    }
}
impl TryFrom<NewFaceRecord> for ActiveModel {
    type Error = AppError;

    fn try_from(value: NewFaceRecord) -> Result<Self, Self::Error> {
        let bbox = serde_json::to_value(&value.bbox)
            .trace_internal_err("db:photo:face:bbox_to:err", "BoundingBox序列化错误")?;
        let landmarks = serde_json::to_value(&value.landmarks)
            .trace_internal_err("db:photo:face:landmarks_to:err", "Landmarks序列化错误")?;

        Ok(ActiveModel {
            id: NotSet,
            photo_id: Set(value.photo_id.0),
            person_id: Set(value.person_id.map(|p| p.0)),
            bbox: Set(bbox),
            landmarks: Set(landmarks),
            score: Set(value.score),
            embedding: Set(PgVector::from(value.embedding)),
            created_at: NotSet,
            updated_at: NotSet,
        })
    }
}
