// ============================================================
// PersonId
// ============================================================

crate::id_type!(PersonId, "photo/");

// ============================================================
// SeaORM 实体（仅 face-engine feature）
// ============================================================

#[cfg(feature = "face-engine")]
mod entity {

    use common::time::DateTime;
    use common::types::changed_value::*;
    use insight_face_rs::{types::FaceEmbedding, BoundingBox};
    use sea_orm::{entity::prelude::*, ActiveValue::Set};
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::photo::{
        face::{FaceId, FaceRecord},
        photo::PhotoId,
        FaceBBox,
    };

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_person")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: PersonId,
        pub name: String,
        pub name_initials: Option<String>,

        pub cover_face_id: FaceId,
        /// 封面人脸所属照片 ID(冗余自 photo_face.photo_id,避免 N+1)
        pub cover_photo_id: PhotoId,
        /// 封面照片 file_id(冗余自 photo_photo.file_id,避免 N+1)
        pub cover_file_id: String,
        /// 封面人脸 score(冗余自 photo_face.score,避免 N+1)
        pub cover_face_score: f32,
        /// 封面人脸归一化 bbox(冗余自 photo_face.bbox,避免 N+1)
        pub cover_bbox: BoundingBox,

        /// score 加权向量和 Σ(score*embedding), 未归一化, 读取时 normalize
        pub centroid: FaceEmbedding,
        pub face_count: i64,
        /// 该人物所有人脸 score 之和(增量维护质心的权重)
        pub weight: f64,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}

    #[derive(Clone, Debug, Serialize)]
    pub struct PersonCover {
        pub face_id: FaceId,
        pub photo_id: PhotoId,
        pub face_score: f32,
        pub file_id: String,
        pub bbox: FaceBBox,
    }
    impl PersonCover {
        pub fn from_face(face: &FaceRecord, file_id: String) -> Self {
            PersonCover {
                face_id: face.id,
                photo_id: face.photo_id,
                face_score: face.score,
                file_id,
                bbox: face.bbox.into(),
            }
        }
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct PersonRecord {
        pub id: PersonId,
        pub name: String,
        pub name_initials: Option<String>,
        pub cover: PersonCover,
        pub centroid: FaceEmbedding,
        pub face_count: u64,
        pub weight: f64,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    #[derive(Debug)]
    pub struct UpdatePersonRecord {
        pub id: PersonId,
        pub name: HasChanged<String>,
        pub name_initials: HasChanged<Option<String>>,
        pub cover: HasChanged<PersonCover>,
        pub centroid: HasChanged<FaceEmbedding>,
        pub face_count: HasChanged<u64>,
        pub weight: HasChanged<f64>,
    }
    impl UpdatePersonRecord {
        pub fn new(id: PersonId) -> Self {
            Self {
                id,
                name: HasChanged::Unchanged,
                name_initials: HasChanged::Unchanged,
                cover: HasChanged::Unchanged,
                centroid: HasChanged::Unchanged,
                face_count: HasChanged::Unchanged,
                weight: HasChanged::Unchanged,
            }
        }

        pub fn with_cover_face(&mut self, cover_face: &FaceRecord, cover_file_id: String) -> &Self {
            self.cover = HasChanged::Changed(PersonCover::from_face(cover_face, cover_file_id));
            self
        }
    }

    impl From<Model> for PersonRecord {
        fn from(value: Model) -> Self {
            Self {
                id: value.id,
                name: value.name,
                name_initials: value.name_initials,
                cover: PersonCover {
                    face_id: value.cover_face_id,
                    photo_id: value.cover_photo_id,
                    face_score: value.cover_face_score,
                    file_id: value.cover_file_id,
                    bbox: value.cover_bbox.into(),
                },
                centroid: value.centroid,
                face_count: value.face_count as u64,
                weight: value.weight,
                created_at: value.created_at,
                updated_at: value.updated_at,
            }
        }
    }

    pub struct NewPerson {
        pub name: String,
        pub cover: PersonCover,
        pub face_count: u64,
        /// 该人物所有人脸 score 之和(增量维护质心的权重)
        pub weight: f64,
        pub centroid: FaceEmbedding,
    }

    impl From<NewPerson> for ActiveModel {
        fn from(value: NewPerson) -> Self {
            ActiveModel {
                name: Set(value.name),
                cover_face_id: Set(value.cover.face_id),
                cover_photo_id: Set(value.cover.photo_id),
                cover_face_score: Set(value.cover.face_score),
                cover_file_id: Set(value.cover.file_id),
                cover_bbox: Set(value.cover.bbox.into()),
                face_count: Set(value.face_count as i64),
                weight: Set(value.weight),
                centroid: Set(value.centroid),
                ..Default::default()
            }
        }
    }
}

#[cfg(feature = "face-engine")]
pub use entity::*;
