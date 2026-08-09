//! photo 域内部共享类型

use serde::{Deserialize, Serialize};
use types::photo::{FaceBBox, person::PersonId};

/// 人物列表视图所需的最小字段集（不含人脸向量），用于三级缓存
///
/// 封面字段直接冗余自 `photo_person.cover_*` 列，避免人物列表查询 N+1。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonBriefRow {
    pub id: PersonId,
    pub name: String,
    pub cover_file_id: String,
    pub cover_bbox: FaceBBox,
    pub face_count: i64,
}
