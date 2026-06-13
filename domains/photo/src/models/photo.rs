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

impl From<PhotoRecord> for PhotoResult {
    fn from(record: PhotoRecord) -> Self {
        Self {
            id: record.id.0.to_string(),
            name: record.name,
            width: record.width,
            height: record.height,
            size: record.size,
            created_at: record.created_at,
            is_favorited: None,
            is_collected: None,
            thumbnail_token: None,
            preview_token: None,
            original_token: None,
        }
    }
}

impl PhotoResult {
    pub fn with_favorited(mut self, is_favorited: bool) -> Self {
        self.is_favorited = Some(is_favorited);
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

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase", default)]
pub struct PhotoCursorParam {
    pub cursor: Option<String>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    pub size: u64,
    pub direction: PageDirection,
    pub default_collection_id: Option<String>,
}

impl Default for PhotoCursorParam {
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

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Md5sExistParam {
    #[validate(length(min = 1, max = 128, message = "MD5 数量在 1 到 128 之间"))]
    pub md5s: Vec<String>,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DeletePhotoParam {
    #[validate(length(min = 1, max = 128, message = "照片数量在 1 到 128 之间"))]
    pub photo_ids: Vec<PhotoId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photo_cursor_query_valid() {
        let param = PhotoCursorParam {
            cursor: None,
            size: 50,
            direction: PageDirection::Next,
            default_collection_id: None,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_photo_cursor_query_size_zero() {
        let param = PhotoCursorParam {
            cursor: None,
            size: 0,
            direction: PageDirection::Next,
            default_collection_id: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_photo_cursor_query_size_too_large() {
        let param = PhotoCursorParam {
            cursor: None,
            size: 1025,
            direction: PageDirection::Next,
            default_collection_id: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_photo_cursor_query_size_exact_max() {
        let param = PhotoCursorParam {
            cursor: None,
            size: 1024,
            direction: PageDirection::Next,
            default_collection_id: None,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_md5s_exist_param_valid() {
        let param = Md5sExistParam {
            md5s: vec!["abc123".to_string(), "def456".to_string()],
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_md5s_exist_param_empty() {
        let param = Md5sExistParam { md5s: vec![] };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_md5s_exist_param_too_many() {
        let param = Md5sExistParam {
            md5s: (0..129).map(|i| format!("md5_{}", i)).collect(),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_md5s_exist_param_exact_max() {
        let param = Md5sExistParam {
            md5s: (0..128).map(|i| format!("md5_{}", i)).collect(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_delete_photo_param_valid() {
        let param = DeletePhotoParam {
            photo_ids: vec![PhotoId(1), PhotoId(2)],
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_delete_photo_param_empty() {
        let param = DeletePhotoParam { photo_ids: vec![] };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_delete_photo_param_exact_max() {
        let param = DeletePhotoParam {
            photo_ids: (0..128).map(|i| PhotoId(i)).collect(),
        };
        assert!(param.validate().is_ok());
    }
}
