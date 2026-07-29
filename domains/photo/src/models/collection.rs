use common::models::ImageToken;
use img_url_generator::TokenCipher;
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use types::photo::photo::PhotoId;
use types::photo::{collection::CollectionRecord, models::PhotoIds};
use validator::Validate;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CollectionResult {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub photo_count: i64,
    pub cover_token: Option<String>,
    pub cover_photo_id: Option<i64>,
    pub created_at: DateTimeUtc,
}

impl From<CollectionRecord> for CollectionResult {
    fn from(record: CollectionRecord) -> Self {
        CollectionResult {
            id: record.id.0.to_string(),
            name: record.name,
            description: record.description,
            photo_count: record.photo_count,
            cover_token: record.cover_file_id,
            cover_photo_id: record.cover_photo_id,
            created_at: record.created_at,
        }
    }
}

impl CollectionResult {
    pub fn with_generate_cover_token(mut self, cipher: &TokenCipher) -> Self {
        self.cover_token = self.cover_token.as_ref().and_then(|fid| {
            cipher
                .encrypt(&ImageToken::thumbnail(fid.to_string()), None)
                .ok()
        });
        self
    }
}

/// 照片所属收藏夹的简要信息
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhotoCollectionResult {
    pub id: String,
    pub name: String,
}

impl From<CollectionRecord> for PhotoCollectionResult {
    fn from(record: CollectionRecord) -> Self {
        PhotoCollectionResult {
            id: record.id.0.to_string(),
            name: record.name,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCreateParam {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: String,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionUpdateParam {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: Option<String>,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
}

pub const COLLECTION_PHOTO_CURSOR_PAGE_DEFAULT_SIZE: u64 = 32;

fn collection_photo_cursor_page_default_size() -> u64 {
    COLLECTION_PHOTO_CURSOR_PAGE_DEFAULT_SIZE
}

/// 收藏夹照片游标参数（`cursor` 为 `TimeIdCursor<PhotoId>` 的 Base64 编码）
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoCursorPageParam {
    pub cursor: Option<String>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[serde(default = "collection_photo_cursor_page_default_size")]
    pub size: u64,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoAddBatchParam {
    pub photo_ids: PhotoIds,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoAddBatchResult {
    pub new_photo_count: u64,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoRemoveBatchParam {
    pub photo_ids: PhotoIds,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoRemoveBatchResult {
    pub removed_photo_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_create_param_valid() {
        let param = CollectionCreateParam {
            name: "My Album".to_string(),
            description: Some("A test album".to_string()),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_create_param_name_empty() {
        let param = CollectionCreateParam {
            name: "".to_string(),
            description: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_create_param_name_too_long() {
        let param = CollectionCreateParam {
            name: "a".repeat(129),
            description: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_create_param_description_too_long() {
        let param = CollectionCreateParam {
            name: "Album".to_string(),
            description: Some("a".repeat(513)),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_update_param_valid() {
        let param = CollectionUpdateParam {
            name: Some("New Name".to_string()),
            description: Some("New desc".to_string()),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_update_param_name_too_long() {
        let param = CollectionUpdateParam {
            name: Some("a".repeat(129)),
            description: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_photo_add_batch_param_valid() {
        let param = CollectionPhotoAddBatchParam {
            photo_ids: PhotoIds::new(vec![PhotoId(1), PhotoId(2)]).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_photo_add_batch_param_empty() {
        let result = PhotoIds::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_collection_photo_cursor_page_query_valid() {
        let param = CollectionPhotoCursorPageParam {
            cursor: None,
            size: 50,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_photo_cursor_page_query_size_too_large() {
        let param = CollectionPhotoCursorPageParam {
            cursor: None,
            size: 1025,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_photo_remove_batch_param_valid() {
        let param = CollectionPhotoRemoveBatchParam {
            photo_ids: PhotoIds::new(vec![PhotoId(1), PhotoId(2)]).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_photo_remove_batch_param_empty() {
        let result = PhotoIds::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_collection_create_param_name_exact_max() {
        let param = CollectionCreateParam {
            name: "a".repeat(128),
            description: None,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_photo_add_batch_param_exact_max() {
        let param = CollectionPhotoAddBatchParam {
            photo_ids: PhotoIds::new((0..1024).map(PhotoId).collect()).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_photo_cursor_page_query_exact_max() {
        let param = CollectionPhotoCursorPageParam {
            cursor: None,
            size: 1024,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_photo_cursor_page_query_size_zero() {
        let param = CollectionPhotoCursorPageParam {
            cursor: None,
            size: 0,
        };
        assert!(param.validate().is_err());
    }
}
