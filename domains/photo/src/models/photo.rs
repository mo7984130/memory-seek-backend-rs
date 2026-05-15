use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use common::models::ImageToken;
use common::utils::TokenCipher;
use common::{error::AppError, utils::ResultExt};
use sea_orm::FromQueryResult;
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
use crate::services::photo_service::PageDirection;

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

impl PhotoVO {
    pub fn generate_tokens(
        file_id: &str,
        token_cipher: &TokenCipher,
    ) -> (Option<String>, Option<String>, Option<String>) {
        let thumbnail_token = token_cipher
            .encrypt(&ImageToken::thumbnail(file_id.to_string()), Some(file_id))
            .ok();
        let preview_token = token_cipher
            .encrypt(&ImageToken::preview(file_id.to_string()), Some(file_id))
            .ok();
        let original_token = token_cipher
            .encrypt(&ImageToken::original(file_id.to_string()), Some(file_id))
            .ok();

        (thumbnail_token, preview_token, original_token)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoCursorQuery {
    pub cursor: Option<String>,
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(default = "default_direction")]
    pub direction: PageDirection,
    pub default_collection_id: Option<String>,
}

fn default_size() -> u32 {
    100
}

fn default_direction() -> PageDirection {
    PageDirection::Next
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoCursor {
    pub created_at: DateTime<Utc>,
    pub id: i64,
}

impl PhotoCursor {
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    pub fn decode(s: impl AsRef<[u8]>) -> Result<Self, AppError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s)
            .trace_bad_request_err("photo::photo_cursor:decode_err", "解码photo_curosr错误")?;
        let json = String::from_utf8(bytes)
            .trace_bad_request_err("photo::photo_cursor:from_utf8_err", "解码photo_curosr错误")?;
        serde_json::from_str(&json)
            .trace_bad_request_err("photo::photo_cursor:from_str_err", "解码photo_curosr错误")
    }
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
    pub md5: Vec<String>,
}

#[derive(Serialize, FromQueryResult)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    pub min_time: Option<DateTimeUtc>,
    pub max_time: Option<DateTimeUtc>,
}
impl Default for TimeRange {
    fn default() -> Self {
        Self {
            min_time: None,
            max_time: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoInfo {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub width: i32,
    pub height: i32,
    pub mime_type: String,
    pub created_at: DateTimeUtc,
}
impl From<entities::photo::Model> for PhotoInfo {
    fn from(m: entities::photo::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            size: m.size,
            width: m.width,
            height: m.height,
            mime_type: m.mime_type,
            created_at: m.created_at,
        }
    }
}
