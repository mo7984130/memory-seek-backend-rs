// ============================================================
// FaceId
// ============================================================

crate::id_type!(FaceId, "photo/");

// ============================================================
// SeaORM 实体（仅 face-engine feature）
// ============================================================

#[cfg(feature = "face-engine")]
mod entity {
    use common::error::AppError;
    use common::ext::ResultErrExt;
    use insight_face_rs::types::{BoundingBox, Face, FaceEmbedding, FaceLandmarks};
    use insight_face_rs::PgVector;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::photo::person::PersonId;
    use crate::photo::photo::PhotoId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_face")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: FaceId,
        pub photo_id: PhotoId,
        pub person_id: Option<PersonId>,

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

    #[derive(Serialize)]
    pub struct FaceRecord {
        pub id: FaceId,
        pub photo_id: PhotoId,
        pub person_id: Option<PersonId>,

        /// 归一化边界框,坐标范围 [0,1](insight-face-rs 2.x 起输出相对坐标)
        pub bbox: BoundingBox,
        /// 归一化关键点(5 点),坐标范围 [0,1]
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

            Ok(Self {
                id: value.id,
                photo_id: value.photo_id,
                person_id: value.person_id,
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
        /// 归一化边界框,坐标范围 [0,1](直接透传检测结果)
        pub bbox: BoundingBox,
        /// 归一化关键点(5 点),坐标范围 [0,1]
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

    impl From<NewFaceRecord> for ActiveModel {
        fn from(record: NewFaceRecord) -> Self {
            use sea_orm::ActiveValue::{NotSet, Set};
            Self {
                id: NotSet,
                photo_id: Set(record.photo_id),
                person_id: Set(record.person_id),
                bbox: Set(serde_json::to_value(record.bbox).unwrap()),
                landmarks: Set(serde_json::to_value(record.landmarks).unwrap()),
                score: Set(record.score),
                embedding: Set(record.embedding.into()),
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
            }
        }
    }
}

#[cfg(feature = "face-engine")]
pub use entity::*;
