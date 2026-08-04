//! 人物封面冗余字段维护(与 `photo_person.cover_*` 列对应)
//!
//! 封面决策规则: 封面人脸 = 该人物下 score 最高的人脸(与聚类 `inner_full_scan` 语义一致)。
//! 集合每次只增减一个元素, 因此封面极值只需两两比较, 无需全量重扫。

use common::{Result, error::AppError, ext::OptionExt};
use sea_orm::ConnectionTrait;
use types::photo::{dto::face::bbox_from_insight, face::FaceRecord};

use crate::mappers::{person_mapper::PersonCoverUpdate, photo_mapper::PhotoMapper};

/// 由封面人脸构造封面冗余字段(`cover_photo_id` / `cover_file_id` / `cover_bbox`)
///
/// 返回 `Ok(None)` 表示封面不变; 此处总是返回 `Some`, `None` 由调用方决策。
pub(crate) async fn cover_update_from_face(
    txn: &impl ConnectionTrait,
    face: &FaceRecord,
) -> Result<Option<PersonCoverUpdate>> {
    let file_id = PhotoMapper::query_file_id_by_id(txn, face.photo_id)
        .await?
        .ok_or_error(
            "person_cover_photo_not_found",
            "封面人脸所属照片不存在",
            AppError::InternalServerError,
        )?;

    Ok(Some(PersonCoverUpdate {
        cover_face_id: face.id,
        cover_photo_id: face.photo_id,
        cover_file_id: file_id,
        cover_bbox: bbox_from_insight(face.bbox),
    }))
}
