use common::time::DateTime;
use validator::Validate;

use crate::auth::user::UserId;
use crate::cursor::TimeIdCursor;
use crate::photo::comment::CommentId;
#[cfg(feature = "orm")]
use crate::photo::comment::CommentRecord;
use crate::photo::models::CommentContent;

pub const COMMENT_CURSOR_PAGE_DEFAULT_SIZE: u64 = 32;
#[allow(dead_code)]
pub const COMMENT_CURSOR_PAGE_MAX_SIZE: u64 = 128;

/// 返回评论分页的默认页大小.
fn comment_cursor_page_default_size() -> u64 {
    COMMENT_CURSOR_PAGE_DEFAULT_SIZE
}

/// 热门评论配置
pub const HOT_COMMENT_MIN_LIKES: u64 = 5;
pub const HOT_COMMENT_MAX_COUNT: u64 = 3;

crate::out_dto!(CommentView, "photo/", rename = "Comment"; {
    pub id: CommentId,
    pub user_id: UserId,
    pub content: String,
    pub like_count: u32,
    pub is_liked: bool,
    pub created_at: DateTime,
});

#[cfg(feature = "orm")]
impl From<CommentRecord> for CommentView {
    fn from(record: CommentRecord) -> Self {
        Self {
            id: record.id,
            user_id: record.user_id,
            content: record.content,
            like_count: record.like_count,
            is_liked: false,
            created_at: record.created_at,
        }
    }
}

impl CommentView {
    /// 写入当前用户对评论的点赞状态.
    pub fn with_liked(mut self, is_like: bool) -> Self {
        self.is_liked = is_like;
        self
    }
}

crate::in_dto!(CommentPublishParam, "photo/"; {
    #[validate(nested)]
    pub content: CommentContent,
});

crate::in_dto!(CommentCursorPageParam, "photo/", docs = "评论游标参数（cursor 为 TimeIdCursor<CommentId> 的 Base64 编码）"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<CommentId>>,
    #[validate(range(min = 1, max = 128, message = "分页大小在 1 到 128 之间"))]
    #[serde(default = "comment_cursor_page_default_size")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_comment_publish_param_valid() {
        let param = CommentPublishParam {
            content: CommentContent::new("This is a comment".to_string()).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_publish_param_empty() {
        let result = CommentContent::new("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_comment_publish_param_too_long() {
        let result = CommentContent::new("a".repeat(1025));
        assert!(result.is_err());
    }

    #[test]
    fn test_comment_publish_param_exact_max() {
        let param = CommentPublishParam {
            content: CommentContent::new("a".repeat(1024)).unwrap(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_cursor_page_query_valid() {
        let param = CommentCursorPageParam {
            cursor: None,
            size: 50,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_cursor_page_query_size_too_large() {
        let param = CommentCursorPageParam {
            cursor: None,
            size: 129,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_comment_cursor_page_query_size_exact_max() {
        let param = CommentCursorPageParam {
            cursor: None,
            size: 128,
        };
        assert!(param.validate().is_ok());
    }
}
