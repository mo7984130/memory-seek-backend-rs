use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use common::Result;
use common::ext::ResultErrExt;
use common::models::ImageToken;
use common::utils::TokenCipher;
use entities::photo::photo::{Model, PhotoId};
use sea_orm::entity::prelude::DateTimeUtc;
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
    pub fn from(photo: Model) -> Self {
        Self {
            id: photo.id.to_string(),
            name: photo.name,
            width: photo.width,
            height: photo.height,
            size: photo.size,
            created_at: photo.created_at,
            is_favorited: None,
            is_collected: None,
            thumbnail_token: None,
            preview_token: None,
            original_token: None,
        }
    }

    pub fn with_favorited(mut self, is_favorited: bool) -> Self {
        self.is_favorited = Some(is_favorited);
        self
    }

    pub fn with_collected(mut self, is_collected: bool) -> Self {
        self.is_collected = Some(is_collected);
        self
    }

    pub fn with_tokens(mut self, token_cipher: &TokenCipher) -> Self {
        self = self.with_original_token(token_cipher);
        self = self.with_thumbnail_token(token_cipher);
        self = self.with_preview_token(token_cipher);
        self
    }

    pub fn with_thumbnail_token(mut self, token_cipher: &TokenCipher) -> Self {
        self.thumbnail_token = token_cipher
            .encrypt(&ImageToken::thumbnail(self.id.to_string()), Some(&self.id))
            .ok();
        self
    }

    pub fn with_preview_token(mut self, token_cipher: &TokenCipher) -> Self {
        self.preview_token = token_cipher
            .encrypt(&ImageToken::preview(self.id.to_string()), Some(&self.id))
            .ok();
        self
    }

    pub fn with_original_token(mut self, token_cipher: &TokenCipher) -> Self {
        self.original_token = token_cipher
            .encrypt(&ImageToken::original(self.id.to_string()), Some(&self.id))
            .ok();
        self
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhotoCursorQuery {
    pub cursor: Option<String>,
    pub size: u64,
    pub direction: PageDirection,
    pub default_collection_id: Option<String>,
}

impl Default for PhotoCursorQuery {
    fn default() -> Self {
        Self {
            cursor: None,
            size: 128,
            direction: PageDirection::Next,
            default_collection_id: None,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadWithCreatedAtQuery {
    pub created_at: DateTimeUtc,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5sExistParam {
    pub md5s: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePhotoParam {
    pub photo_ids: Vec<PhotoId>,
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
impl From<Model> for PhotoInfo {
    /// 从数据库照片实体转换为照片信息 DTO
    fn from(m: Model) -> Self {
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
