use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhotoVO {
    pub id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub size: i64,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_favorited: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_collected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoCursorQuery {
    pub cursor: Option<DateTime<Utc>>,
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(default = "default_direction")]
    pub direction: String,
    pub default_collection_id: Option<String>,
}

fn default_size() -> u32 {
    100
}

fn default_direction() -> String {
    "next".to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPageVO<T, C> {
    pub records: Vec<T>,
    pub next_cursor: Option<C>,
    pub has_more: bool,
}

impl<T, C> CursorPageVO<T, C> {
    pub fn empty() -> Self {
        Self {
            records: vec![],
            next_cursor: None,
            has_more: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadWithCreatedAtQuery {
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5Query {
    pub md5: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeRangeVO {
    pub min: DateTime<Utc>,
    pub max: DateTime<Utc>,
}
