// ============================================================
// FaceId
// ============================================================

crate::id_type!(FaceId, "photo/");

// ============================================================
// SeaORM 实体（仅 face-engine feature）
// ============================================================

#[cfg(feature = "face-engine")]
mod entity {
    use common::time::{now, DateTime};
    use insight_face_rs::types::{BoundingBox, Face, FaceEmbedding, FaceLandmarks};
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

        pub bbox: BoundingBox,
        pub landmarks: FaceLandmarks,
        pub score: f32,

        pub embedding: FaceEmbedding,

        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}

    #[derive(Serialize, Clone)]
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
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    impl From<Model> for FaceRecord {
        fn from(value: Model) -> Self {
            Self {
                id: value.id,
                photo_id: value.photo_id,
                person_id: value.person_id,
                bbox: value.bbox,
                landmarks: value.landmarks,
                score: value.score,
                embedding: value.embedding,
                created_at: value.created_at,
                updated_at: value.updated_at,
            }
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
        /// 将人脸检测结果转换为待持久化的人脸记录.
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
                bbox: Set(record.bbox),
                landmarks: Set(record.landmarks),
                score: Set(record.score),
                embedding: Set(record.embedding),
                created_at: Set(now()),
                updated_at: Set(now()),
            }
        }
    }
}

#[cfg(feature = "face-engine")]
pub use entity::*;
