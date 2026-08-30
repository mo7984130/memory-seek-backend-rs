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
    use common::types::HasChanged;
    use insight_face_rs::{BoundingBox, types::FaceEmbedding};
    use sea_orm::{ActiveValue::Set, entity::prelude::*};
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::photo::{
        FaceBBox,
        face::{FaceId, FaceRecord},
        photo::PhotoId,
    };

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_person")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: PersonId,

        /// 名称
        /// 索引, 按照人脸名称获取人物
        #[sea_orm(indexed)]
        pub name: String,
        /// 名称首字母
        /// 如果名称为数字等判断不出来首字母的情况下, 为空
        /// 索引, 按照首字母搜索人物
        #[sea_orm(indexed)]
        pub name_initials: Option<String>,

        /// 封面的人脸 ID
        pub cover_face_id: FaceId,
        /// 封面人脸 所属照片 ID
        pub cover_photo_id: PhotoId,
        /// 封面照片 file_id
        pub cover_file_id: String,
        /// 封面人脸 score
        pub cover_face_score: f32,
        /// 封面人脸 bbox
        pub cover_bbox: BoundingBox,

        /// 人物的加权向量
        pub centroid: FaceEmbedding,
        /// 人脸总数
        /// 索引, 按照人脸总数排序获取人物列表
        #[sea_orm(indexed)]
        pub face_count: u64,
        /// 总权重, 为人脸score之和
        pub weight: f64,

        /// 更新时间
        pub updated_at: DateTime,

        /// 创建时间
        pub created_at: DateTime,
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
                face_count: value.face_count,
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
                face_count: Set(value.face_count),
                weight: Set(value.weight),
                centroid: Set(value.centroid),
                ..Default::default()
            }
        }
    }
}

#[cfg(feature = "face-engine")]
pub use entity::*;
