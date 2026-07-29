//! 照片相关类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use validator::Validate;

use crate::auth::user::UserId;
use crate::photo::collection::CollectionId;
use crate::photo::comment::CommentId;
use crate::photo::photo::PhotoId;

// ============================================================
// PhotoIds — 校验型照片 ID 批量列表
// ============================================================

/// 照片 ID 批量列表，构造即保证：非空，且不超过 1024 个
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "Vec<PhotoId>", into = "Vec<PhotoId>")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(type = "Array<PhotoId>"))]
pub struct PhotoIds(Vec<PhotoId>);

impl PhotoIds {
    pub const MAX_COUNT: usize = 1024;

    pub fn new(ids: Vec<PhotoId>) -> Result<Self, &'static str> {
        if ids.is_empty() {
            return Err("照片ID列表不能为空");
        }
        if ids.len() > Self::MAX_COUNT {
            return Err("照片数量不能超过1024");
        }
        Ok(Self(ids))
    }

    pub fn into_inner(self) -> Vec<PhotoId> {
        self.0
    }
}

impl Deref for PhotoIds {
    type Target = [PhotoId];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Vec<PhotoId>> for PhotoIds {
    type Error = &'static str;

    fn try_from(ids: Vec<PhotoId>) -> Result<Self, Self::Error> {
        Self::new(ids)
    }
}

impl From<PhotoIds> for Vec<PhotoId> {
    fn from(ids: PhotoIds) -> Self {
        ids.0
    }
}

impl Validate for PhotoIds {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        // 构造时（反序列化时）已校验，此处为 no-op
        Ok(())
    }
}

// ============================================================
// CommentContent — 校验型评论内容
// ============================================================

/// 评论内容，构造即保证：非空，且不超过 1024 个字符
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(type = "string"))]
pub struct CommentContent(String);

impl CommentContent {
    pub const MAX_LEN: usize = 1024;

    pub fn new(content: String) -> Result<Self, &'static str> {
        if content.is_empty() {
            return Err("评论内容不能为空");
        }
        if content.len() > Self::MAX_LEN {
            return Err("评论内容不能超过1024个字符");
        }
        Ok(Self(content))
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for CommentContent {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<String> for CommentContent {
    type Error = &'static str;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<CommentContent> for String {
    fn from(c: CommentContent) -> Self {
        c.0
    }
}

impl Validate for CommentContent {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        Ok(())
    }
}

/// 上传照片请求参数（文件的二进制数据由 multipart 单独传递）
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Serialize, Deserialize)]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Serialize, Deserialize)]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCommentParam {
    /// 评论内容
    pub content: CommentContent,
}

/// 批量检查 MD5 是否存在
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistsByMd5BatchParam {
    /// MD5 值列表，数量限制 1~128
    #[validate(length(min = 1, max = 128, message = "MD5 数量在 1 到 128 之间"))]
    pub md5s: Vec<String>,
}

/// 删除照片
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePhotosParam {
    /// 照片 ID 列表
    pub photo_ids: PhotoIds,
}

/// 收藏夹添加照片
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCollectionPhotosParam {
    /// 收藏夹 ID
    pub collection_id: i64,

    /// 照片 ID 列表
    pub photo_ids: PhotoIds,
}

/// 收藏夹移除照片
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveCollectionPhotosParam {
    /// 收藏夹 ID
    pub collection_id: i64,

    /// 照片 ID 列表
    pub photo_ids: PhotoIds,
}

/// 评论游标查询参数
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Deserialize)]
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

/// 用户点赞照片列表查询参数
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Validate, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LikedPhotosQuery {
    /// 分页游标（可选，首次查询不传）
    pub cursor: Option<String>,

    /// 每页大小（可选，默认 20，最大 100）
    #[validate(range(min = 1, max = 100, message = "size 在 1 到 100 之间"))]
    pub size: Option<u64>,
}

/// 照片信息响应
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoDTO {
    /// 照片 ID
    pub id: PhotoId,
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
    pub uploader_id: UserId,
    /// 创建时间
    pub created_at: String,
}

/// 收藏夹信息响应
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDTO {
    /// 收藏夹 ID
    pub id: CollectionId,
    /// 收藏夹名称
    pub name: String,
    /// 收藏夹描述
    pub description: Option<String>,
    /// 照片数量
    pub photo_count: i64,
    /// 创建者 ID
    pub creator_id: UserId,
    /// 创建时间
    pub created_at: String,
}

/// 评论信息响应
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentDTO {
    /// 评论 ID
    pub id: CommentId,
    /// 评论内容
    pub content: String,
    /// 评论者 ID
    pub commenter_id: UserId,
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
    fn test_photo_ids_validate_is_noop() {
        let ids = PhotoIds::new(vec![PhotoId(1)]).unwrap();
        assert!(ids.validate().is_ok());
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
        let result = serde_json::from_str::<DeletePhotosParam>(json);
        assert!(result.is_err());
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
            id: PhotoId(123),
            title: "Sunset".to_string(),
            description: Some("A sunset".to_string()),
            url: "https://example.com/photo.jpg".to_string(),
            thumbnail_url: "https://example.com/thumb.jpg".to_string(),
            tags: vec!["sunset".to_string()],
            uploader_id: UserId(123),
            created_at: "2026-06-13T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&photo).unwrap();
        assert!(json.contains("\"thumbnailUrl\""));
        assert!(json.contains("\"uploaderId\""));
        assert!(json.contains("\"createdAt\""));
    }
}
