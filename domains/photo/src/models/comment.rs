use entities::photo::comment::CommentRecord;
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub const COMMENT_CURSOR_PAGE_MAX_SIZE: u64 = 128;

/// 热门评论配置
pub const HOT_COMMENT_MIN_LIKES: u64 = 5;
pub const HOT_COMMENT_MAX_COUNT: u64 = 3;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhotoCommentVO {
    pub id: String,
    pub user_id: String,
    pub content: String,
    pub like_count: i32,
    pub is_liked: bool,
    pub created_at: DateTimeUtc,
}
impl From<CommentRecord> for PhotoCommentVO {
    fn from(record: CommentRecord) -> Self {
        Self {
            id: record.id.0.to_string(),
            user_id: record.user_id.to_string(),
            content: record.content,
            like_count: record.like_count,
            is_liked: false,
            created_at: record.created_at,
        }
    }
}

impl PhotoCommentVO {
    pub fn with_liked(mut self, is_like: bool) -> Self {
        self.is_liked = is_like;
        self
    }
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CommentPublishParam {
    #[validate(length(min = 1, max = 1024, message = "评论内容长度在 1 到 1024 个字符"))]
    pub content: String,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CommentCursorPageQuery {
    pub cursor: Option<DateTimeUtc>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    pub size: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment_publish_param_valid() {
        let param = CommentPublishParam {
            content: "This is a comment".to_string(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_publish_param_empty() {
        let param = CommentPublishParam {
            content: "".to_string(),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_comment_publish_param_too_long() {
        let param = CommentPublishParam {
            content: "a".repeat(1025),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_comment_publish_param_exact_max() {
        let param = CommentPublishParam {
            content: "a".repeat(1024),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_cursor_page_query_valid() {
        let param = CommentCursorPageQuery {
            cursor: None,
            size: Some(50),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_cursor_page_query_size_too_large() {
        let param = CommentCursorPageQuery {
            cursor: None,
            size: Some(1025),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_comment_cursor_page_query_size_exact_max() {
        let param = CommentCursorPageQuery {
            cursor: None,
            size: Some(1024),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_cursor_page_query_size_zero() {
        let param = CommentCursorPageQuery {
            cursor: None,
            size: Some(0),
        };
        assert!(param.validate().is_err());
    }
}
