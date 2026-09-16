use crate::cursor::TimeIdCursor;
use common::time::DateTime;
use validator::Validate;

use crate::media::collection::CollectionId;

use crate::media::models::MediaIds;
use crate::media::media::MediaId;

crate::out_dto!(CollectionView, "media/", rename = "Collection"; {
    pub id: CollectionId,
    pub name: String,
    pub description: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub media_count: u64,
    pub cover_token: Option<String>,
    /// 封面照片 ID（字符串）
    pub cover_media_id: Option<MediaId>,
    pub created_at: DateTime,
});

crate::out_dto!(CollectionBriefView, "media/", rename = "CollectionBrief"; {
    pub id: CollectionId,
    pub name: String,
});

crate::in_dto!(CollectionCreateParam, "media/"; {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: String,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
});

crate::in_dto!(CollectionUpdateParam, "media/"; {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: Option<String>,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
});

pub const COLLECTION_PHOTO_CURSOR_PAGE_DEFAULT_SIZE: u64 = 32;

/// 返回相册照片分页的默认页大小.
fn collection_media_cursor_page_default_size() -> u64 {
    COLLECTION_PHOTO_CURSOR_PAGE_DEFAULT_SIZE
}

crate::in_dto!(CollectionMediaCursorPageParam, "media/", docs = "收藏夹照片游标参数（cursor 为 TimeIdCursor<MediaId> 的 Base64 编码）"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<MediaId>>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[serde(default = "collection_media_cursor_page_default_size")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

crate::in_dto!(CollectionMediaAddBatchParam, "media/"; {
    #[validate(nested)]
    pub media_ids: MediaIds,
});

crate::out_dto!(CollectionMediaAddBatchResult, "media/", Default; {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub new_media_count: u64,
});

crate::in_dto!(CollectionMediaRemoveBatchParam, "media/"; {
    #[validate(nested)]
    pub media_ids: MediaIds,
});

crate::out_dto!(CollectionMediaRemoveBatchResult, "media/", Default; {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub removed_media_count: u64,
});

#[cfg(feature = "orm")]
mod orm {
    use super::*;

    use crate::auth::user::UserId;
    use crate::media::collection::CollectionRecord;
    use crate::media::{CollectionBriefView, ImageToken};

    impl From<CollectionRecord> for CollectionBriefView {
        fn from(record: CollectionRecord) -> Self {
            CollectionBriefView {
                id: record.id,
                name: record.name,
            }
        }
    }

    impl CollectionView {
        /// 为相册封面生成当前查看者可用的裁剪访问令牌.
        pub fn with_generate_cover_token(
            mut self,
            viewer: UserId,
        ) -> common::error::contextual::Result<Self> {
            self.cover_token = self
                .cover_token
                .as_ref()
                .map(|fid| ImageToken::thumbnail(viewer, fid.to_string()).encrypt())
                .transpose()?;
            Ok(self)
        }
    }

    impl From<CollectionRecord> for CollectionView {
        fn from(record: CollectionRecord) -> Self {
            CollectionView {
                id: record.id,
                name: record.name,
                description: record.description,
                media_count: record.media_count,
                cover_token: record.cover_file_id,
                cover_media_id: record.cover_media_id,
                created_at: record.created_at,
            }
        }
    }
}

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
    fn test_collection_media_add_batch_param_valid() {
        let param = CollectionMediaAddBatchParam {
            media_ids: MediaIds::new(vec![MediaId(1), MediaId(2)]).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_media_add_batch_param_empty() {
        let result = MediaIds::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_collection_media_cursor_page_query_valid() {
        let param = CollectionMediaCursorPageParam {
            cursor: None,
            size: 50,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_media_cursor_page_query_size_too_large() {
        let param = CollectionMediaCursorPageParam {
            cursor: None,
            size: 1025,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_media_remove_batch_param_valid() {
        let param = CollectionMediaRemoveBatchParam {
            media_ids: MediaIds::new(vec![MediaId(1), MediaId(2)]).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_media_remove_batch_param_empty() {
        let result = MediaIds::new(vec![]);
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
    fn test_collection_media_add_batch_param_exact_max() {
        let param = CollectionMediaAddBatchParam {
            media_ids: MediaIds::new((0..1024).map(MediaId).collect()).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_media_cursor_page_query_exact_max() {
        let param = CollectionMediaCursorPageParam {
            cursor: None,
            size: 1024,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_media_cursor_page_query_size_zero() {
        let param = CollectionMediaCursorPageParam {
            cursor: None,
            size: 0,
        };
        assert!(param.validate().is_err());
    }
}
