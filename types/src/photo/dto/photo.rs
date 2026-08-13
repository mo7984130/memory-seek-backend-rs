use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::auth::user::UserId;
use crate::cursor::TimeIdCursor;
use crate::photo::photo::PhotoId;
#[cfg(feature = "orm")]
use crate::photo::photo::PhotoRecord;
#[cfg(feature = "orm")]
use crate::photo::ImageToken;
#[cfg(feature = "orm")]
use common::utils::TokenCipher;

crate::out_dto!(PhotoView, "photo/", rename = "Photo"; {
    pub id: PhotoId,
    pub user_id: UserId,
    pub name: String,
    pub width: i32,
    pub height: i32,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
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
});

#[cfg(feature = "orm")]
impl From<PhotoRecord> for PhotoView {
    fn from(record: PhotoRecord) -> Self {
        Self {
            id: record.id,
            user_id: record.user_id,
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

impl PhotoView {
    pub fn with_liked(mut self, is_liked: bool) -> Self {
        self.is_liked = Some(is_liked);
        self
    }

    #[cfg(feature = "orm")]
    pub fn with_tokens(
        mut self,
        file_id: &str,
        viewer: UserId,
        token_cipher: &TokenCipher,
    ) -> common::error::deferred::Result<Self> {
        self = self.with_original_token(file_id, viewer, token_cipher)?;
        self = self.with_thumbnail_token(file_id, viewer, token_cipher)?;
        self = self.with_preview_token(file_id, viewer, token_cipher)?;
        Ok(self)
    }

    #[cfg(feature = "orm")]
    pub fn with_thumbnail_token(
        mut self,
        file_id: &str,
        viewer: UserId,
        token_cipher: &TokenCipher,
    ) -> common::error::deferred::Result<Self> {
        self.thumbnail_token = Some(token_cipher.encrypt(
            &ImageToken::thumbnail(viewer, file_id),
            Some(&format!("{}:{}", self.id, viewer)),
        )?);
        Ok(self)
    }

    #[cfg(feature = "orm")]
    pub fn with_preview_token(
        mut self,
        file_id: &str,
        viewer: UserId,
        token_cipher: &TokenCipher,
    ) -> common::error::deferred::Result<Self> {
        self.preview_token = Some(token_cipher.encrypt(
            &ImageToken::preview(viewer, file_id),
            Some(&format!("{}:{}", self.id, viewer)),
        )?);
        Ok(self)
    }

    #[cfg(feature = "orm")]
    pub fn with_original_token(
        mut self,
        file_id: &str,
        viewer: UserId,
        token_cipher: &TokenCipher,
    ) -> common::error::deferred::Result<Self> {
        self.original_token = Some(token_cipher.encrypt(
            &ImageToken::original(viewer, file_id),
            Some(&format!("{}:{}", self.id, viewer)),
        )?);
        Ok(self)
    }
}

crate::in_dto!(PhotoCursorParam, "photo/", serde_default, docs = "照片游标参数（cursor 为 TimeIdCursor<PhotoId> 的 Base64 编码）"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<PhotoId>>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
    pub direction: PageDirection,
    pub anchor_time: Option<DateTime<Utc>>,
});

impl Default for PhotoCursorParam {
    fn default() -> Self {
        Self {
            cursor: None,
            size: 32,
            direction: PageDirection::Next,
            anchor_time: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "photo/"))]
pub enum PageDirection {
    Next,
    Prev,
}
