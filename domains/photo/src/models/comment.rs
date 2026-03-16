use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishCommentDTO {
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentPageQuery {
    pub cursor: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}
