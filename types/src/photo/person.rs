// ============================================================
// PersonId
// ============================================================

crate::id_type!(PersonId, "photo/");

// ============================================================
// SeaORM 实体（仅 face-engine feature）
// ============================================================

#[cfg(feature = "face-engine")]
mod entity {
    use common::error::AppError;
    use common::ext::ResultErrExt;
    use insight_face_rs::types::FaceEmbedding;
    use insight_face_rs::PgVector;
    use sea_orm::{entity::prelude::*, ActiveValue::Set};
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::photo::{face::FaceId, photo::PhotoId, FaceBBox};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_person")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub name: String,
        pub name_initials: Option<String>,
        pub cover_face_id: i64,
        /// 封面人脸所属照片 ID(冗余自 photo_face.photo_id,避免 N+1)
        pub cover_photo_id: i64,
        /// 封面照片 file_id(冗余自 photo_photo.file_id,避免 N+1)
        pub cover_file_id: String,
        /// 封面人脸归一化 bbox(冗余自 photo_face.bbox,避免 N+1)
        #[sea_orm(column_type = "Json")]
        pub cover_bbox: Json,
        /// score 加权向量和 Σ(score*embedding), 未归一化, 读取时 normalize
        /// (增量维护, 见 docs/change-face-belonging-plan.md)
        pub centroid: PgVector,
        pub face_count: i64,
        /// 该人物所有人脸 score 之和(增量维护质心的权重)
        pub weight: f64,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}

    #[derive(Clone, Debug, Serialize)]
    pub struct PersonRecord {
        pub id: PersonId,
        pub name: String,
        pub name_initials: Option<String>,
        pub cover_face_id: FaceId,
        pub cover_photo_id: PhotoId,
        pub cover_file_id: String,
        pub cover_bbox: FaceBBox,
        pub centroid: FaceEmbedding,
        pub face_count: u64,
        pub weight: f64,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    impl TryFrom<Model> for PersonRecord {
        type Error = AppError;
        fn try_from(value: Model) -> Result<Self, Self::Error> {
            let embedding: FaceEmbedding = value.centroid.into();
            let cover_bbox: FaceBBox = serde_json::from_value(value.cover_bbox)
                .trace_internal_err("db:photo:person:cover_bbox_from:err", "封面 bbox 转换错误")?;

            Ok(Self {
                id: PersonId(value.id),
                name: value.name,
                name_initials: value.name_initials,
                cover_face_id: FaceId(value.cover_face_id),
                cover_photo_id: PhotoId(value.cover_photo_id),
                cover_file_id: value.cover_file_id,
                cover_bbox,
                centroid: embedding,
                face_count: value.face_count as u64,
                weight: value.weight,
                created_at: value.created_at,
                updated_at: value.updated_at,
            })
        }
    }

    pub struct NewPerson {
        pub name: String,
        pub cover_face_id: FaceId,
        pub cover_photo_id: PhotoId,
        pub cover_file_id: String,
        pub cover_bbox: FaceBBox,
        pub face_count: u64,
        /// 该人物所有人脸 score 之和(增量维护质心的权重)
        pub weight: f64,
        pub centroid: FaceEmbedding,
    }

    impl From<NewPerson> for ActiveModel {
        fn from(value: NewPerson) -> Self {
            ActiveModel {
                name: Set(value.name),
                cover_face_id: Set(value.cover_face_id.0),
                cover_photo_id: Set(value.cover_photo_id.0),
                cover_file_id: Set(value.cover_file_id),
                cover_bbox: Set(serde_json::to_value(value.cover_bbox).unwrap()),
                face_count: Set(value.face_count as i64),
                weight: Set(value.weight),
                centroid: Set(value.centroid.into()),
                ..Default::default()
            }
        }
    }
}

#[cfg(feature = "face-engine")]
pub use entity::*;
