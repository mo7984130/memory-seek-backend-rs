use crate::photo::face::{FaceId, FaceRecord};
use crate::photo::models::FaceIds;
use crate::photo::person::PersonId;
use crate::photo::photo::PhotoId;
use validator::Validate;

/// 人脸边界框（归一化坐标）——统一复用 `crate::photo::image_token::FaceBBox`
use crate::photo::FaceBBox;

/// insight_face_rs 边界框 → 项目统一归一化边界框
///
/// `FaceBBox` 定义在 `common`，无法在 `types` 内实现孤儿规则的 `From`，
/// 因此以转换函数形式提供。
pub fn bbox_from_insight(v: insight_face_rs::BoundingBox) -> FaceBBox {
    FaceBBox {
        x1: v.x1,
        y1: v.y1,
        x2: v.x2,
        y2: v.y2,
    }
}

/// 项目统一归一化边界框 → insight_face_rs 边界框
pub fn bbox_to_insight(v: FaceBBox) -> insight_face_rs::BoundingBox {
    insight_face_rs::BoundingBox {
        x1: v.x1,
        y1: v.y1,
        x2: v.x2,
        y2: v.y2,
    }
}

crate::out_dto!(FaceView, "photo/", rename = "Face"; {
    pub id: FaceId,
    pub bbox: FaceBBox,
    /// 归属人物 ID（未分配时为 null）
    pub person_id: Option<PersonId>,
    /// 归属人物名称（未分配时为 null）
    pub person_name: Option<String>,
});

impl From<FaceRecord> for FaceView {
    fn from(value: FaceRecord) -> Self {
        Self {
            id: value.id,
            bbox: bbox_from_insight(value.bbox),
            person_id: value.person_id,
            person_name: None,
        }
    }
}

fn unassigned_face_photo_cursor_page_default_size() -> u64 {
    32
}

crate::in_dto!(UnassignedFacePhotoCursorParam, "photo/", docs = "未分配人脸照片游标参数(cursor 为 TimeIdCursor<PhotoId> 的 Base64 编码)"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<crate::cursor::TimeIdCursor<PhotoId>>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[serde(default = "unassigned_face_photo_cursor_page_default_size")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

crate::in_dto!(FaceDeleteBatchParam, "photo/", docs = "批量删除人脸参数（仅限未归属人脸）"; {
    #[validate(nested)]
    pub face_ids: FaceIds,
});

crate::out_dto!(FaceDeleteBatchResult, "photo/", Default; {
    /// 实际删除的人脸数量
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub deleted_face_count: u64,
});

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// insight-face-rs 2.x 输出归一化坐标 [0,1],FaceBBox 应直接透传
    #[test]
    fn face_bbox_passthrough_normalized_coords() {
        let bbox = insight_face_rs::BoundingBox {
            x1: 0.1,
            y1: 0.2,
            x2: 0.55,
            y2: 0.9,
        };
        let dto = bbox_from_insight(bbox);
        assert!(approx(dto.x1, 0.1));
        assert!(approx(dto.y1, 0.2));
        assert!(approx(dto.x2, 0.55));
        assert!(approx(dto.y2, 0.9));

        let back = bbox_to_insight(dto);
        assert!(approx(back.x1, 0.1));
        assert!(approx(back.y1, 0.2));
    }

    /// 库 2.x 提供的 相对坐标 <-> 绝对像素 转换(裁剪链路依赖)
    #[test]
    fn bounding_box_relative_absolute_roundtrip() {
        let abs = insight_face_rs::BoundingBox {
            x1: 10.0,
            y1: 20.0,
            x2: 100.0,
            y2: 200.0,
        };
        let rel = abs.to_relative(200, 400);
        assert!(approx(rel.x1, 0.05));
        assert!(approx(rel.y1, 0.05));
        assert!(approx(rel.x2, 0.5));
        assert!(approx(rel.y2, 0.5));

        let back = rel.to_absolute(200, 400);
        assert!(approx(back.x1, 10.0));
        assert!(approx(back.y1, 20.0));
        assert!(approx(back.x2, 100.0));
        assert!(approx(back.y2, 200.0));
    }

    // ==================== FaceDeleteBatchParam ====================

    #[test]
    fn face_delete_batch_param_deserialize_valid() {
        let json = r#"{"faceIds": [1, 2]}"#;
        let param: FaceDeleteBatchParam = serde_json::from_str(json).unwrap();
        assert_eq!(param.face_ids.len(), 2);
        assert!(param.validate().is_ok());
    }

    #[test]
    fn face_delete_batch_param_deserialize_empty_then_validate() {
        // 空列表应能反序列化, 校验错误走 validator 通道
        let json = r#"{"faceIds": []}"#;
        let param: FaceDeleteBatchParam = serde_json::from_str(json).unwrap();
        assert!(param.validate().is_err());
    }

    #[test]
    fn face_delete_batch_param_deserialize_too_many_then_validate() {
        let ids = (0..1025).collect::<Vec<_>>();
        let json = format!(r#"{{"faceIds": {:?}}}"#, ids);
        let param: FaceDeleteBatchParam = serde_json::from_str(&json).unwrap();
        assert!(param.validate().is_err());
    }
}
