use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use img_url_generator::{ImageToken, encrypt_image_token};
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

impl PhotoVO {
    pub fn generate_tokens(
        file_id: &str,
        encryption_key: &[u8; 32],
    ) -> (Option<String>, Option<String>, Option<String>) {
        let thumbnail_token =
            encrypt_image_token(&ImageToken::thumbnail(file_id.to_string()), encryption_key).ok();
        let preview_token =
            encrypt_image_token(&ImageToken::preview(file_id.to_string()), encryption_key).ok();
        let original_token =
            encrypt_image_token(&ImageToken::original(file_id.to_string()), encryption_key).ok();

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
    pub direction: String,
    pub default_collection_id: Option<String>,
}

fn default_size() -> u32 {
    100
}

fn default_direction() -> String {
    "next".to_string()
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

    pub fn decode(s: impl AsRef<[u8]>) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(s).ok()?;
        let json = String::from_utf8(bytes).ok()?;
        serde_json::from_str(&json).ok()
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
    pub md5: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeRangeVO {
    pub min: DateTime<Utc>,
    pub max: DateTime<Utc>,
}
