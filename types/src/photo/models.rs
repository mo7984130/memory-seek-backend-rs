//! 照片相关类型定义
use validator::Validate;

use crate::cursor::TimeIdCursor;
use crate::photo::face::FaceId;
use crate::photo::photo::PhotoId;

// ============================================================
// PhotoIds — 校验型照片 ID 批量列表
// ============================================================

crate::validated_newtype!(
    PhotoIds,
    Vec<PhotoId>,
    1024,
    "photo/",
    "照片ID列表不能为空",
    "照片数量不能超过1024"
);

// ============================================================
// FaceIds — 校验型人脸 ID 批量列表
// ============================================================

crate::validated_newtype!(
    FaceIds,
    Vec<FaceId>,
    1024,
    "photo/",
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
    "photo/",
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
    "photo/",
    "人物名称不能为空",
    "人物名称长度不能超过64个字符"
);

crate::in_dto!(UploadPhotoParam, "photo/", serialize, docs = "上传照片请求参数（文件的二进制数据由 multipart 单独传递）"; {
    /// 文件名
    #[validate(length(min = 1, max = 255, message = "文件名长度在 1 到 255 个字符"))]
    pub file_name: String,

    /// 文件 MIME 类型
    #[validate(length(min = 1, max = 100, message = "文件类型不能为空"))]
    pub content_type: String,
});

crate::in_dto!(ExistsByMd5BatchParam, "photo/", serialize; {
    /// MD5 值列表，数量限制 1~128
    #[validate(length(min = 1, max = 128, message = "MD5 数量在 1 到 128 之间"))]
    pub md5s: Vec<String>,
});

crate::in_dto!(DeletePhotosParam, "photo/", serialize; {
    /// 照片 ID 列表
    #[validate(nested)]
    pub photo_ids: PhotoIds,
});

/// 返回点赞照片分页的默认页大小.
fn liked_photos_default_size() -> u64 {
    32
}

crate::in_dto!(LikedPhotosQuery, "photo/"; {
    /// 分页游标（可选，首次查询不传）
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<PhotoId>>,

    /// 每页大小（可选，默认 32，最大 100）
    #[serde(default = "liked_photos_default_size")]
    #[validate(range(min = 1, max = 128, message = "size 在 1 到 128 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    // ==================== PhotoIds construction ====================

    #[test]
    fn test_photo_ids_new_valid() {
        let ids = PhotoIds::new(vec![PhotoId(1), PhotoId(2)]);
        assert!(ids.is_ok());
    }

    #[test]
    fn test_photo_ids_new_empty() {
        let ids = PhotoIds::new(vec![]);
        assert!(ids.is_err());
    }

    #[test]
    fn test_photo_ids_new_too_many() {
        let ids = PhotoIds::new((0..1025).map(PhotoId).collect());
        assert!(ids.is_err());
    }

    #[test]
    fn test_photo_ids_new_exact_max() {
        let ids = PhotoIds::new((0..1024).map(PhotoId).collect());
        assert!(ids.is_ok());
    }

    #[test]
    fn test_photo_ids_validate_empty() {
        let ids = PhotoIds::new(vec![]).unwrap_err();
        assert_eq!(ids, "照片ID列表不能为空");
        let ids = PhotoIds(vec![]);
        assert!(ids.validate().is_err());
    }

    #[test]
    fn test_photo_ids_validate_too_many() {
        let ids = PhotoIds((0..1025).map(PhotoId).collect());
        assert!(ids.validate().is_err());
    }

    #[test]
    fn test_photo_ids_deref() {
        let ids = PhotoIds::new(vec![PhotoId(1), PhotoId(2)]).unwrap();
        // Deref to &[PhotoId]
        let slice: &[PhotoId] = &ids;
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0], PhotoId(1));
    }

    // ==================== CommentContent construction ====================

    #[test]
    fn test_comment_content_new_valid() {
        let c = CommentContent::new("Great photo!".to_string());
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

    // ==================== UploadPhotoParam validation ====================

    #[test]
    fn test_upload_photo_param_valid() {
        let param = UploadPhotoParam {
            file_name: "photo.jpg".to_string(),
            content_type: "image/jpeg".to_string(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_upload_photo_param_invalid_empty_file_name() {
        let param = UploadPhotoParam {
            file_name: "".to_string(),
            content_type: "image/jpeg".to_string(),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_upload_photo_param_invalid_empty_content_type() {
        let param = UploadPhotoParam {
            file_name: "photo.jpg".to_string(),
            content_type: "".to_string(),
        };
        assert!(param.validate().is_err());
    }

    // ==================== ExistsByMd5BatchParam validation ====================

    #[test]
    fn test_exists_by_md5_batch_param_valid() {
        let param = ExistsByMd5BatchParam {
            md5s: vec!["abc123".to_string(), "def456".to_string()],
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_exists_by_md5_batch_param_empty() {
        let param = ExistsByMd5BatchParam { md5s: vec![] };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_exists_by_md5_batch_param_too_many() {
        let param = ExistsByMd5BatchParam {
            md5s: (0..129).map(|i| format!("md5_{}", i)).collect(),
        };
        assert!(param.validate().is_err());
    }

    // ==================== DeletePhotosParam serde ====================

    #[test]
    fn test_delete_photos_param_deserialize_valid() {
        let json = r#"{"photoIds": [1, 2]}"#;
        let param: DeletePhotosParam = serde_json::from_str(json).unwrap();
        assert_eq!(param.photo_ids.len(), 2);
    }

    #[test]
    fn test_delete_photos_param_deserialize_empty() {
        let json = r#"{"photoIds": []}"#;
        let param: DeletePhotosParam = serde_json::from_str(json).unwrap();
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_delete_photos_param_deserialize_too_many() {
        let ids = (0..1025).map(|_| 1).collect::<Vec<_>>();
        let json = format!(r#"{{"photoIds": {:?}}}"#, ids);
        let param: DeletePhotosParam = serde_json::from_str(&json).unwrap();
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_person_name_deserialize_then_validate() {
        // 超长名称应能反序列化, 校验错误走 validator 通道, 不含位置信息
        let json = format!(r#"{{"newName": "{}"}}"#, "a".repeat(65));
        let param: crate::photo::dto::person::RenamePersonParam =
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
