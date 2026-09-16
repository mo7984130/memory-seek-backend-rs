use crate::cursor::TimeIdCursor;
use common::time::DateTime;
use validator::Validate;

use crate::visual::collection::CollectionId;

use crate::visual::models::VisualIds;
use crate::visual::visual::VisualId;

crate::out_dto!(CollectionView, "visual/", rename = "Collection"; {
    pub id: CollectionId,
    pub name: String,
    pub description: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub visual_count: u64,
    pub cover_token: Option<String>,
    /// 封面影像 ID（字符串）
    pub cover_visual_id: Option<VisualId>,
    pub created_at: DateTime,
});

crate::out_dto!(CollectionBriefView, "visual/", rename = "CollectionBrief"; {
    pub id: CollectionId,
    pub name: String,
});

crate::in_dto!(CollectionCreateParam, "visual/"; {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: String,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
});

crate::in_dto!(CollectionUpdateParam, "visual/"; {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: Option<String>,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
});

pub const COLLECTION_PHOTO_CURSOR_PAGE_DEFAULT_SIZE: u64 = 32;

/// 返回相册影像分页的默认页大小.
fn collection_visual_cursor_page_default_size() -> u64 {
    COLLECTION_PHOTO_CURSOR_PAGE_DEFAULT_SIZE
}

crate::in_dto!(CollectionVisualCursorPageParam, "visual/", docs = "收藏夹影像游标参数（cursor 为 TimeIdCursor<VisualId> 的 Base64 编码）"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<VisualId>>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[serde(default = "collection_visual_cursor_page_default_size")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

crate::in_dto!(CollectionVisualAddBatchParam, "visual/"; {
    #[validate(nested)]
    pub visual_ids: VisualIds,
});

crate::out_dto!(CollectionVisualAddBatchResult, "visual/", Default; {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub new_visual_count: u64,
});

crate::in_dto!(CollectionVisualRemoveBatchParam, "visual/"; {
    #[validate(nested)]
    pub visual_ids: VisualIds,
});

crate::out_dto!(CollectionVisualRemoveBatchResult, "visual/", Default; {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub removed_visual_count: u64,
});

#[cfg(feature = "orm")]
mod orm {
    use super::*;

    use crate::auth::user::UserId;
    use crate::visual::collection::CollectionRecord;
    use crate::visual::{CollectionBriefView, ImageToken};

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
                visual_count: record.visual_count,
                cover_token: record.cover_file_id,
                cover_visual_id: record.cover_visual_id,
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
    fn test_collection_visual_add_batch_param_valid() {
        let param = CollectionVisualAddBatchParam {
            visual_ids: VisualIds::new(vec![VisualId(1), VisualId(2)]).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_visual_add_batch_param_empty() {
        let result = VisualIds::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_collection_visual_cursor_page_query_valid() {
        let param = CollectionVisualCursorPageParam {
            cursor: None,
            size: 50,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_visual_cursor_page_query_size_too_large() {
        let param = CollectionVisualCursorPageParam {
            cursor: None,
            size: 1025,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_visual_remove_batch_param_valid() {
        let param = CollectionVisualRemoveBatchParam {
            visual_ids: VisualIds::new(vec![VisualId(1), VisualId(2)]).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_visual_remove_batch_param_empty() {
        let result = VisualIds::new(vec![]);
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
    fn test_collection_visual_add_batch_param_exact_max() {
        let param = CollectionVisualAddBatchParam {
            visual_ids: VisualIds::new((0..1024).map(VisualId).collect()).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_visual_cursor_page_query_exact_max() {
        let param = CollectionVisualCursorPageParam {
            cursor: None,
            size: 1024,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_visual_cursor_page_query_size_zero() {
        let param = CollectionVisualCursorPageParam {
            cursor: None,
            size: 0,
        };
        assert!(param.validate().is_err());
    }
}
