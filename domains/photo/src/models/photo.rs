use chrono::{DateTime, Utc};
use common::models::ImageToken;
use common::utils::TokenCipher;
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
use types::photo::photo::PhotoRecord;
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

/// 照片游标参数（`cursor` 为 `TimeIdCursor<PhotoId>` 的 Base64 编码）
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

#[derive(Debug, Clone, PartialEq, Eq, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageDirection {
    Next,
    Prev,
}
