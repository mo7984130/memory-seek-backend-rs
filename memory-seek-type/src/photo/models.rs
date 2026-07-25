//! 照片相关类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use validator::Validate;

/// 上传照片请求参数（文件的二进制数据由 multipart 单独传递）
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UploadPhotoParam {
    /// 文件名
    #[validate(length(min = 1, max = 255, message = "文件名长度在 1 到 255 个字符"))]
    pub file_name: String,

    /// 文件 MIME 类型
    #[validate(length(min = 1, max = 100, message = "文件类型不能为空"))]
    pub content_type: String,

    /// 自定义创建时间（可选）
    pub created_at: Option<DateTime<Utc>>,
}

/// 创建收藏夹请求参数
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionParam {
    /// 收藏夹名称
    #[validate(length(min = 1, max = 50, message = "名称长度在 1 到 50 个字符"))]
    pub name: String,

    /// 收藏夹描述
    #[validate(length(max = 200, message = "描述长度最多 200 个字符"))]
    pub description: Option<String>,
}

/// 添加评论请求参数
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AddCommentParam {
    /// 评论内容
    #[validate(length(min = 1, max = 500, message = "评论长度在 1 到 500 个字符"))]
    pub content: String,
}

/// 批量检查 MD5 是否存在
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ExistsByMd5BatchParam {
    /// MD5 值列表，数量限制 1~128
    #[validate(length(min = 1, max = 128, message = "MD5 数量在 1 到 128 之间"))]
    pub md5s: Vec<String>,
}

/// 删除照片
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DeletePhotosParam {
    /// 照片 ID 列表，数量限制 1~128
    #[validate(length(min = 1, max = 128, message = "照片数量在 1 到 128 之间"))]
    pub photo_ids: Vec<String>,
}

/// 收藏夹添加照片
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AddCollectionPhotosParam {
    /// 收藏夹 ID
    pub collection_id: i64,

    /// 照片 ID 列表
    #[validate(length(min = 1, max = 128, message = "照片数量在 1 到 128 之间"))]
    pub photo_ids: Vec<String>,
}

/// 收藏夹移除照片
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RemoveCollectionPhotosParam {
    /// 收藏夹 ID
    pub collection_id: i64,

    /// 照片 ID 列表
    #[validate(length(min = 1, max = 128, message = "照片数量在 1 到 128 之间"))]
    pub photo_ids: Vec<String>,
}

/// 评论游标查询参数
#[derive(Debug, Validate, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CommentCursorParam {
    /// 照片 ID
    pub photo_id: i64,

    /// 分页游标（可选，首次查询不传）
    pub cursor: Option<DateTime<Utc>>,

    /// 每页大小（可选，默认 32）
    #[validate(range(min = 1, max = 128, message = "size 在 1 到 128 之间"))]
    pub size: Option<u64>,
}

/// 照片信息响应（保留兼容，后续对齐实际后端）
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PhotoDTO {
    /// 照片 ID
    pub id: String,
    /// 照片标题
    pub title: String,
    /// 照片描述
    pub description: Option<String>,
    /// 照片 URL
    pub url: String,
    /// 缩略图 URL
    pub thumbnail_url: String,
    /// 标签
    pub tags: Vec<String>,
    /// 上传者 ID
    pub uploader_id: String,
    /// 创建时间
    pub created_at: String,
}

/// 收藏夹信息响应（保留兼容）
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDTO {
    /// 收藏夹 ID
    pub id: String,
    /// 收藏夹名称
    pub name: String,
    /// 收藏夹描述
    pub description: Option<String>,
    /// 照片数量
    pub photo_count: i64,
    /// 创建者 ID
    pub creator_id: String,
    /// 创建时间
    pub created_at: String,
}

/// 评论信息响应（保留兼容）
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CommentDTO {
    /// 评论 ID
    pub id: String,
    /// 评论内容
    pub content: String,
    /// 评论者 ID
    pub commenter_id: String,
    /// 评论者昵称
    pub commenter_nickname: String,
    /// 评论者头像
    pub commenter_avatar: Option<String>,
    /// 创建时间
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== UploadPhotoParam validation ====================

    #[test]
    fn test_upload_photo_param_valid() {
        let param = UploadPhotoParam {
            file_name: "photo.jpg".to_string(),
            content_type: "image/jpeg".to_string(),
            created_at: None,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_upload_photo_param_invalid_empty_file_name() {
        let param = UploadPhotoParam {
            file_name: "".to_string(),
            content_type: "image/jpeg".to_string(),
            created_at: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_upload_photo_param_invalid_empty_content_type() {
        let param = UploadPhotoParam {
            file_name: "photo.jpg".to_string(),
            content_type: "".to_string(),
            created_at: None,
        };
        assert!(param.validate().is_err());
    }

    // ==================== CreateCollectionParam validation ====================

    #[test]
    fn test_create_collection_param_valid() {
        let param = CreateCollectionParam {
            name: "My Collection".to_string(),
            description: Some("A collection of photos".to_string()),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_create_collection_param_invalid_empty_name() {
        let param = CreateCollectionParam {
            name: "".to_string(),
            description: None,
        };
        assert!(param.validate().is_err());
    }

    // ==================== AddCommentParam validation ====================

    #[test]
    fn test_add_comment_param_valid() {
        let param = AddCommentParam {
            content: "Great photo!".to_string(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_add_comment_param_invalid_empty_content() {
        let param = AddCommentParam {
            content: "".to_string(),
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

    // ==================== DeletePhotosParam validation ====================

    #[test]
    fn test_delete_photos_param_valid() {
        let param = DeletePhotosParam {
            photo_ids: vec!["1".to_string(), "2".to_string()],
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_delete_photos_param_empty() {
        let param = DeletePhotosParam { photo_ids: vec![] };
        assert!(param.validate().is_err());
    }

    // ==================== CommentCursorParam validation ====================

    #[test]
    fn test_comment_cursor_param_default_size() {
        let param = CommentCursorParam {
            photo_id: 1,
            cursor: None,
            size: None,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_cursor_param_valid_size() {
        let param = CommentCursorParam {
            photo_id: 1,
            cursor: None,
            size: Some(50),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_cursor_param_invalid_size() {
        let param = CommentCursorParam {
            photo_id: 1,
            cursor: None,
            size: Some(200),
        };
        assert!(param.validate().is_err());
    }

    // ==================== DTO serialization ====================

    #[test]
    fn test_photo_dto_serializes_to_camel_case() {
        let photo = PhotoDTO {
            id: "123".to_string(),
            title: "Sunset".to_string(),
            description: Some("A sunset".to_string()),
            url: "https://example.com/photo.jpg".to_string(),
            thumbnail_url: "https://example.com/thumb.jpg".to_string(),
            tags: vec!["sunset".to_string()],
            uploader_id: "user123".to_string(),
            created_at: "2026-06-13T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&photo).unwrap();
        assert!(json.contains("\"thumbnailUrl\""));
        assert!(json.contains("\"uploaderId\""));
        assert!(json.contains("\"createdAt\""));
    }
}
