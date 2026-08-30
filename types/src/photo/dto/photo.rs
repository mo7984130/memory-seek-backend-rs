use common::time::DateTime;
use serde::Deserialize;

use crate::auth::user::UserId;
use crate::cursor::TimeIdCursor;
#[cfg(feature = "orm")]
use crate::photo::ImageToken;
use crate::photo::photo::PhotoId;
#[cfg(feature = "orm")]
use crate::photo::photo::PhotoRecord;
#[cfg(feature = "orm")]
use common::utils::{TokenCipher, token_cipher};

crate::out_dto!(PhotoView, "photo/", rename = "Photo"; {
    pub id: PhotoId,
    pub user_id: UserId,
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: u64,
    pub created_at: DateTime,
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
    /// 写入当前用户对照片的点赞状态.
    pub fn with_liked(mut self, is_liked: bool) -> Self {
        self.is_liked = Some(is_liked);
        self
    }

    #[cfg(feature = "orm")]
    pub fn from_record_with_tokens(
        record: PhotoRecord,
        viewer: UserId,
    ) -> common::error::contextual::Result<Self> {
        let file_id = record.file_id.clone();
        Self::from(record).with_tokens(&file_id, viewer, token_cipher())
    }

    #[cfg(feature = "orm")]
    pub fn with_tokens(
        mut self,
        file_id: &str,
        viewer: UserId,
        token_cipher: &TokenCipher,
    ) -> common::error::contextual::Result<Self> {
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
    ) -> common::error::contextual::Result<Self> {
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
    ) -> common::error::contextual::Result<Self> {
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
    ) -> common::error::contextual::Result<Self> {
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
    pub anchor_time: Option<DateTime>,
});

#[cfg(all(test, feature = "orm"))]
mod orm_tests {
    use super::*;
    use crate::photo::ImageTokenType;
    use common::utils::{TokenCipherConfig, init_token_cipher};

    fn test_cipher() -> &'static TokenCipher {
        init_token_cipher(&TokenCipherConfig {
            key: "test-key".to_owned(),
            salt: "test-salt".to_owned(),
        })
    }

    fn photo_record() -> PhotoRecord {
        PhotoRecord {
            id: PhotoId(1),
            user_id: UserId(2),
            name: "photo.jpg".to_owned(),
            size: 1024,
            width: 640,
            height: 480,
            mime_type: "image/jpeg".to_owned(),
            md5: "md5".to_owned(),
            file_id: "file-id".to_owned(),
            comment_count: 0,
            like_count: 0,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    fn assert_token(token: &str, cipher: &TokenCipher, viewer: UserId, token_type: ImageTokenType) {
        let token = cipher.decrypt::<ImageToken>(token).unwrap();
        assert_eq!(token.file_id, "file-id");
        assert_eq!(token.viewer_id, viewer);
        assert_eq!(token.token_type, token_type);
        assert!(token.bbox.is_none());
    }

    #[test]
    fn from_record_with_tokens_preserves_fields_and_generates_tokens() {
        let viewer = UserId(3);
        let cipher = test_cipher();
        let record = photo_record();
        let expected_created_at = record.created_at;

        let view = PhotoView::from_record_with_tokens(record, viewer).unwrap();

        assert_eq!(view.id, PhotoId(1));
        assert_eq!(view.user_id, UserId(2));
        assert_eq!(view.name, "photo.jpg");
        assert_eq!(view.created_at, expected_created_at);
        assert_token(
            view.thumbnail_token.as_ref().unwrap(),
            cipher,
            viewer,
            ImageTokenType::Thumbnail,
        );
        assert_token(
            view.preview_token.as_ref().unwrap(),
            cipher,
            viewer,
            ImageTokenType::Preview,
        );
        assert_token(
            view.original_token.as_ref().unwrap(),
            cipher,
            viewer,
            ImageTokenType::Original,
        );
    }
}

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
