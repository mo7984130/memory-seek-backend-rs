// ============================================================
// FaceId
// ============================================================

crate::id_type!(FaceId, "photo/");

// ============================================================
// SeaORM 实体（仅 face-engine feature）
// ============================================================

#[cfg(feature = "face-engine")]
mod entity {
    use common::time::{DateTime, now};
    use insight_face_rs::types::{BoundingBox, Face, FaceEmbedding, FaceLandmarks};
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::photo::person::PersonId;
    use crate::photo::photo::PhotoId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_face")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: FaceId,

        /// 所在的照片ID
        /// 索引, 用于 查询照片里面的人脸
        #[sea_orm(indexed)]
        pub photo_id: PhotoId,

        /// 所属的人物ID
        /// 可为空, 即 无归属
        /// 索引, 用于 查询人物的人脸
        #[sea_orm(indexed)]
        pub person_id: Option<PersonId>,

        /// BBox, 相对定位
        pub bbox: BoundingBox,
        /// 五点定位, 相对定位
        pub landmarks: FaceLandmarks,
        /// 置信度
        pub score: f32,
        /// 向量
        pub embedding: FaceEmbedding,

        /// 更新时间
        pub updated_at: DateTime,

        /// 创建时间
        pub created_at: DateTime,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}

    #[derive(Serialize, Clone)]
    pub struct FaceRecord {
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
