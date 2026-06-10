use chrono::{DateTime, Utc};
use entities::photo::comment::Model;
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
    pub created_at: DateTime<Utc>,
}
impl From<Model> for PhotoCommentVO {
    fn from(model: Model) -> Self {
        Self {
            id: model.id.to_string(),
            user_id: model.user_id.to_string(),
            content: model.content,
            like_count: model.like_count,
            is_liked: false,
            created_at: model.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishCommentDTO {
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentPageQuery {
    pub cursor: Option<DateTimeUtc>,
    pub limit: Option<i64>,
}
