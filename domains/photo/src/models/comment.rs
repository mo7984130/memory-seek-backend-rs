use chrono::{DateTime, Utc};
use entities::photo::comment::CommentRecord;
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentPublishParam {
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentCursorPageQuery {
    pub cursor: Option<DateTimeUtc>,
    pub size: Option<u64>,
}
