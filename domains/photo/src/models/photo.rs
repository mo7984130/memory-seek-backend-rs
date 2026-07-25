use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use common::Result;
use common::ext::ResultErrExt;
use common::models::ImageToken;
use common::utils::TokenCipher;
use entities::photo::photo::{PhotoId, PhotoRecord};
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhotoResult {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub size: i64,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_collected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_liked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_token: Option<String>,
}

impl From<PhotoRecord> for PhotoResult {
    fn from(record: PhotoRecord) -> Self {
        Self {
            id: record.id.to_string(),
            user_id: record.user_id.to_string(),
            name: record.name,
            width: record.width,
            height: record.height,
            size: record.size,
            created_at: record.created_at,
            is_collected: None,
            is_liked: None,
            thumbnail_token: None,
            preview_token: None,
            original_token: None,
        }
    }
}

impl PhotoResult {
    pub fn with_liked(mut self, is_liked: bool) -> Self {
        self.is_liked = Some(is_liked);
        self
    }

    pub fn with_tokens(mut self, file_id: &str, token_cipher: &TokenCipher) -> Self {
        self = self.with_original_token(file_id, token_cipher);
        self = self.with_thumbnail_token(file_id, token_cipher);
        self = self.with_preview_token(file_id, token_cipher);
        self
    }

    pub fn with_thumbnail_token(mut self, file_id: &str, token_cipher: &TokenCipher) -> Self {
        self.thumbnail_token = token_cipher
            .encrypt(&ImageToken::thumbnail(file_id), Some(&self.id))
            .ok();
        self
    }

    pub fn with_preview_token(mut self, file_id: &str, token_cipher: &TokenCipher) -> Self {
        self.preview_token = token_cipher
            .encrypt(&ImageToken::preview(file_id), Some(&self.id))
            .ok();
        self
    }

    pub fn with_original_token(mut self, file_id: &str, token_cipher: &TokenCipher) -> Self {
        self.original_token = token_cipher
            .encrypt(&ImageToken::original(file_id), Some(&self.id))
            .ok();
        self
    }
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase", default)]
pub struct PhotoCursorParam {
    pub cursor: Option<String>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    pub size: u64,
    pub direction: PageDirection,
    pub default_collection_id: Option<String>,
    pub anchor_time: Option<DateTimeUtc>,
}

impl Default for PhotoCursorParam {
    fn default() -> Self {
        Self {
            cursor: None,
            size: 128,
            direction: PageDirection::Next,
            default_collection_id: None,
            anchor_time: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoCursor {
    pub created_at: DateTimeUtc,
    pub id: PhotoId,
}

impl PhotoCursor {
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    pub fn decode(s: impl AsRef<[u8]>) -> Result<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(s).trace_warn_bad_request(
            "photo_cursor:decode_err",
            "解码photo_curosr错误, base64解码失败",
            "解码photo_curosr错误",
        )?;
        let json = String::from_utf8(bytes).trace_warn_bad_request(
            "photo_cursor:from_utf8_err",
            "解码photo_curosr错误, bytes转String错误",
            "解码photo_curosr错误",
        )?;
        serde_json::from_str(&json).trace_warn_bad_request(
            "photo_cursor:from_str_err",
            "解码photo_curosr错误, json解析失败",
            "解码photo_curosr错误",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageDirection {
    Next,
    Prev,
}
