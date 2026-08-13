use chrono::{DateTime, Utc};
use validator::Validate;

use crate::auth::user::UserId;
use crate::cursor::TimeIdCursor;
use crate::photo::collection::CollectionId;
#[cfg(feature = "orm")]
use crate::photo::collection::CollectionRecord;
use crate::photo::models::PhotoIds;
use crate::photo::photo::PhotoId;
#[cfg(feature = "orm")]
use crate::photo::ImageToken;
#[cfg(feature = "orm")]
use common::utils::TokenCipher;

crate::out_dto!(CollectionView, "photo/", rename = "Collection"; {
    pub id: CollectionId,
    pub name: String,
    pub description: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub photo_count: i64,
    pub cover_token: Option<String>,
    /// 封面照片 ID（字符串）
    pub cover_photo_id: Option<PhotoId>,
    pub created_at: DateTime<Utc>,
});

#[cfg(feature = "orm")]
impl From<CollectionRecord> for CollectionView {
    fn from(record: CollectionRecord) -> Self {
        CollectionView {
            id: record.id,
            name: record.name,
            description: record.description,
            photo_count: record.photo_count,
            cover_token: record.cover_file_id,
            cover_photo_id: record.cover_photo_id,
            created_at: record.created_at,
        }
    }
}

#[cfg(feature = "orm")]
impl CollectionView {
    pub fn with_generate_cover_token(
        mut self,
        viewer: UserId,
        cipher: &TokenCipher,
    ) -> common::error::deferred::Result<Self> {
        self.cover_token = self
            .cover_token
            .as_ref()
            .map(|fid| {
                let seed = format!("{}:{}", viewer, fid);
                cipher.encrypt(&ImageToken::thumbnail(viewer, fid.to_string()), Some(&seed))
            })
            .transpose()?;
        Ok(self)
    }
}

crate::out_dto!(CollectionBriefView, "photo/", rename = "CollectionBrief"; {
    pub id: CollectionId,
    pub name: String,
});

#[cfg(feature = "orm")]
impl From<CollectionRecord> for CollectionBriefView {
    fn from(record: CollectionRecord) -> Self {
        CollectionBriefView {
            id: record.id,
            name: record.name,
        }
    }
}

crate::in_dto!(CollectionCreateParam, "photo/"; {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: String,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
});

crate::in_dto!(CollectionUpdateParam, "photo/"; {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: Option<String>,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
});

pub const COLLECTION_PHOTO_CURSOR_PAGE_DEFAULT_SIZE: u64 = 32;

fn collection_photo_cursor_page_default_size() -> u64 {
    COLLECTION_PHOTO_CURSOR_PAGE_DEFAULT_SIZE
}

crate::in_dto!(CollectionPhotoCursorPageParam, "photo/", docs = "收藏夹照片游标参数（cursor 为 TimeIdCursor<PhotoId> 的 Base64 编码）"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<PhotoId>>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[serde(default = "collection_photo_cursor_page_default_size")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

crate::in_dto!(CollectionPhotoAddBatchParam, "photo/"; {
    #[validate(nested)]
    pub photo_ids: PhotoIds,
});

crate::out_dto!(CollectionPhotoAddBatchResult, "photo/", Default; {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub new_photo_count: u64,
});

crate::in_dto!(CollectionPhotoRemoveBatchParam, "photo/"; {
    #[validate(nested)]
    pub photo_ids: PhotoIds,
});

crate::out_dto!(CollectionPhotoRemoveBatchResult, "photo/", Default; {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub removed_photo_count: u64,
});

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

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
