//! 影像相关类型定义
use common::time::DateTime;
use validator::Validate;

use crate::cursor::TimeIdCursor;
use crate::visual::face::FaceId;
use crate::visual::visual::VisualId;

// ============================================================
// VisualIds — 校验型影像 ID 批量列表
// ============================================================

crate::validated_newtype!(
    VisualIds,
    Vec<VisualId>,
    1024,
    "visual/",
    "影像ID列表不能为空",
    "影像数量不能超过1024"
);

// ============================================================
// FaceIds — 校验型人脸 ID 批量列表
// ============================================================

crate::validated_newtype!(
    FaceIds,
    Vec<FaceId>,
    1024,
    "visual/",
    "人脸ID列表不能为空",
    "人脸数量不能超过1024"
);

// ============================================================
// CommentContent — 校验型评论内容
// ============================================================

crate::validated_newtype!(
    CommentContent,
    String,
    1024,
    "visual/",
    "评论内容不能为空",
    "评论内容不能超过1024个字符"
);

// ============================================================
// PersonName — 校验型人物名称
// ============================================================

crate::validated_newtype!(
    PersonName,
    String,
    64,
    "visual/",
    "人物名称不能为空",
    "人物名称长度不能超过64个字符"
);

crate::in_dto!(UploadVisualParam, "visual/", serialize, docs = "上传影像请求参数（文件的二进制数据由 multipart 单独传递）"; {
    /// 指定创建时间（可选，仅管理员可设置，RFC3339 格式）
    pub created_at: Option<DateTime>,
});

crate::in_dto!(ExistsByHashBatchParam, "visual/", serialize; {
    /// 哈希值列表，数量限制 1~128
    #[validate(length(min = 1, max = 128, message = "哈希值数量在 1 到 128 之间"))]
    pub hashes: Vec<String>,
});

crate::in_dto!(DeleteVisualsParam, "visual/", serialize; {
    /// 影像 ID 列表
    #[validate(nested)]
    pub visual_ids: VisualIds,
});

/// 返回点赞影像分页的默认页大小.
fn liked_visuals_default_size() -> u64 {
    32
}

crate::in_dto!(LikedVisualsQuery, "visual/"; {
    /// 分页游标（可选，首次查询不传）
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<VisualId>>,

    /// 每页大小（可选，默认 32，最大 100）
    #[serde(default = "liked_visuals_default_size")]
    #[validate(range(min = 1, max = 128, message = "size 在 1 到 128 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    // ==================== VisualIds construction ====================

    #[test]
    fn test_visual_ids_new_valid() {
        let ids = VisualIds::new(vec![VisualId(1), VisualId(2)]);
        assert!(ids.is_ok());
    }

    #[test]
    fn test_visual_ids_new_empty() {
        let ids = VisualIds::new(vec![]);
        assert!(ids.is_err());
    }

    #[test]
    fn test_visual_ids_new_too_many() {
        let ids = VisualIds::new((0..1025).map(VisualId).collect());
        assert!(ids.is_err());
    }

    #[test]
    fn test_visual_ids_new_exact_max() {
        let ids = VisualIds::new((0..1024).map(VisualId).collect());
        assert!(ids.is_ok());
    }

    #[test]
    fn test_visual_ids_validate_empty() {
        let ids = VisualIds::new(vec![]).unwrap_err();
        assert_eq!(ids, "影像ID列表不能为空");
        let ids = VisualIds(vec![]);
        assert!(ids.validate().is_err());
    }

    #[test]
    fn test_visual_ids_validate_too_many() {
        let ids = VisualIds((0..1025).map(VisualId).collect());
        assert!(ids.validate().is_err());
    }

    #[test]
    fn test_visual_ids_deref() {
        let ids = VisualIds::new(vec![VisualId(1), VisualId(2)]).unwrap();
        // Deref to &[VisualId]
        let slice: &[VisualId] = &ids;
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0], VisualId(1));
    }

    // ==================== CommentContent construction ====================

    #[test]
    fn test_comment_content_new_valid() {
        let c = CommentContent::new("Great visual!".to_string());
        assert!(c.is_ok());
    }

    #[test]
    fn test_comment_content_new_empty() {
        let c = CommentContent::new("".to_string());
        assert!(c.is_err());
    }

    #[test]
    fn test_comment_content_new_too_long() {
        let c = CommentContent::new("a".repeat(1025));
        assert!(c.is_err());
    }

    #[test]
    fn test_comment_content_new_exact_max() {
        let c = CommentContent::new("a".repeat(1024));
        assert!(c.is_ok());
    }

    #[test]
    fn test_comment_content_deref() {
        let c = CommentContent::new("hello".to_string()).unwrap();
        // Deref to &str
        let s: &str = &c;
        assert_eq!(s, "hello");
    }

    // ==================== PersonName construction ====================

    #[test]
    fn test_person_name_new_valid() {
        let n = PersonName::new("Alice".to_string());
        assert!(n.is_ok());
    }

    #[test]
    fn test_person_name_new_empty() {
        let n = PersonName::new("".to_string());
        assert!(n.is_err());
    }

    #[test]
    fn test_person_name_new_too_long() {
        let n = PersonName::new("a".repeat(65));
        assert!(n.is_err());
    }

    #[test]
    fn test_person_name_new_exact_max() {
        let n = PersonName::new("a".repeat(64));
        assert!(n.is_ok());
    }

    #[test]
    fn test_person_name_deref() {
        let n = PersonName::new("Alice".to_string()).unwrap();
        let s: &str = &n;
        assert_eq!(s, "Alice");
    }

    #[test]
    fn test_person_name_validate_is_noop() {
        let n = PersonName::new("Alice".to_string()).unwrap();
        assert!(n.validate().is_ok());
    }

    #[test]
    fn test_person_name_validate_empty() {
        let n = PersonName(String::new());
        assert!(n.validate().is_err());
    }

    #[test]
    fn test_person_name_validate_too_long() {
        let n = PersonName("a".repeat(65));
        assert!(n.validate().is_err());
    }

    // ==================== ExistsByHashBatchParam validation ====================

    #[test]
    fn test_exists_by_hash_batch_param_valid() {
        let param = ExistsByHashBatchParam {
            hashes: vec!["abc123".to_string(), "def456".to_string()],
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_exists_by_hash_batch_param_empty() {
        let param = ExistsByHashBatchParam { hashes: vec![] };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_exists_by_hash_batch_param_too_many() {
        let param = ExistsByHashBatchParam {
            hashes: (0..129).map(|i| format!("hash_{}", i)).collect(),
        };
        assert!(param.validate().is_err());
    }

    // ==================== DeleteVisualsParam serde ====================

    #[test]
    fn test_delete_visuals_param_deserialize_valid() {
        let json = r#"{"visualIds": [1, 2]}"#;
        let param: DeleteVisualsParam = serde_json::from_str(json).unwrap();
        assert_eq!(param.visual_ids.len(), 2);
    }

    #[test]
    fn test_delete_visuals_param_deserialize_empty() {
        let json = r#"{"visualIds": []}"#;
        let param: DeleteVisualsParam = serde_json::from_str(json).unwrap();
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_delete_visuals_param_deserialize_too_many() {
        let ids = (0..1025).map(|_| 1).collect::<Vec<_>>();
        let json = format!(r#"{{"visualIds": {:?}}}"#, ids);
        let param: DeleteVisualsParam = serde_json::from_str(&json).unwrap();
        assert!(param.validate().is_err());
    }

    #[cfg(feature = "face-engine")]
    #[test]
    fn test_person_name_deserialize_then_validate() {
        // 超长名称应能反序列化, 校验错误走 validator 通道, 不含位置信息
        let json = format!(r#"{{"newName": "{}"}}"#, "a".repeat(65));
        let param: crate::visual::dto::person::RenamePersonParam =
            serde_json::from_str(&json).unwrap();
        let err = param.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("人物名称长度不能超过64个字符"), "msg: {msg}");
        assert!(!msg.contains("at line"), "msg: {msg}");
    }

    // ==================== FaceIds construction ====================

    #[test]
    fn test_face_ids_new_valid() {
        let ids = FaceIds::new(vec![FaceId(1), FaceId(2)]);
        assert!(ids.is_ok());
    }

    #[test]
    fn test_face_ids_new_empty() {
        let ids = FaceIds::new(vec![]);
        assert!(ids.is_err());
    }

    #[test]
    fn test_face_ids_new_too_many() {
        let ids = FaceIds::new((0..1025).map(FaceId).collect());
        assert!(ids.is_err());
    }

    #[test]
    fn test_face_ids_new_exact_max() {
        let ids = FaceIds::new((0..1024).map(FaceId).collect());
        assert!(ids.is_ok());
    }

    #[test]
    fn test_face_ids_validate_empty() {
        let ids = FaceIds(vec![]);
        assert!(ids.validate().is_err());
    }

    #[test]
    fn test_face_ids_validate_too_many() {
        let ids = FaceIds((0..1025).map(FaceId).collect());
        assert!(ids.validate().is_err());
    }
}
