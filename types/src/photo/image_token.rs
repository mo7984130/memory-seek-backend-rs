//! 统一图片 Token
//!
//! 提供图片访问 token 契约类型（`ImageToken` / `ImageTokenType` / `FaceBBox`），
//! 由 `common` 下沉至 `types`，供 photo / user / auth 各域复用，并可随 DTO 一起导出 TS。

use serde::{Deserialize, Serialize};

use crate::auth::user::UserId;
#[cfg(feature = "orm")]
use common::error::AppError;
#[cfg(feature = "orm")]
use common::ext::ResultErrExt;
#[cfg(feature = "orm")]
use common::utils::TokenCipher;

/// 图片类型
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ImageTokenType {
    Thumbnail,
    Preview,
    Original,
    Crop,
}

/// 人脸边界框（归一化坐标，x1/y1 左上角、x2/y2 右下角，取值范围 0~1）
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "photo/", rename = "FaceBBox"))]
pub struct FaceBBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl FaceBBox {
    /// 将归一化边界框转换为指定图片尺寸下的绝对像素裁剪矩形 `(x, y, w, h)`
    ///
    /// # 参数
    /// - `width`: 原图宽度（像素）
    /// - `height`: 原图高度（像素）
    ///
    /// # 返回
    /// 返回 `(x, y, w, h)`，其中 `(x, y)` 为左上角坐标，`w`/`h` 为裁剪宽高
    pub fn to_pixel_rect(self, width: u32, height: u32) -> (i32, i32, i32, i32) {
        let w = width as f32;
        let h = height as f32;
        let x = (self.x1 * w).round().clamp(0.0, w) as i32;
        let y = (self.y1 * h).round().clamp(0.0, h) as i32;
        let x2 = (self.x2 * w).round().clamp(x as f32, w) as i32;
        let y2 = (self.y2 * h).round().clamp(y as f32, h) as i32;
        (x, y, x2 - x, y2 - y)
    }
}

/// 统一图片 Token
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImageToken {
    /// 文件路径
    pub file_id: String,
    /// 图片类型
    #[serde(rename = "type")]
    pub token_type: ImageTokenType,
    /// 人脸边界框（归一化坐标，仅 Crop 类型需要）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<FaceBBox>,
    /// 浏览者用户 ID（图片访问审计主体）
    pub viewer_id: UserId,
}

impl ImageToken {
    /// 创建缩略图 token
    ///
    /// # 参数
    /// - `viewer_id`: 浏览者用户 ID
    /// - `file_id`: 图片文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Thumbnail` 的 `ImageToken`
    pub fn thumbnail(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            token_type: ImageTokenType::Thumbnail,
            bbox: None,
            viewer_id: viewer,
        }
    }

    /// 创建预览图 token
    ///
    /// # 参数
    /// - `viewer_id`: 浏览者用户 ID
    /// - `file_id`: 图片文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Preview` 的 `ImageToken`
    pub fn preview(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            token_type: ImageTokenType::Preview,
            bbox: None,
            viewer_id: viewer,
        }
    }

    /// 创建原图 token
    ///
    /// # 参数
    /// - `viewer_id`: 浏览者用户 ID
    /// - `file_id`: 图片文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Original` 的 `ImageToken`
    pub fn original(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            token_type: ImageTokenType::Original,
            bbox: None,
            viewer_id: viewer,
        }
    }

    /// 创建裁剪图 token（人脸封面）
    ///
    /// # 参数
    /// - `file_id`: 图片文件 ID
    /// - `bbox`: 人脸边界框（归一化坐标），用于定位裁剪区域
    /// - `viewer_id`: 浏览者用户 ID
    ///
    /// # 返回
    /// 返回类型为 `Crop` 且包含 `bbox` 的 `ImageToken`
    pub fn crop(viewer: UserId, file_id: impl Into<String>, bbox: FaceBBox) -> Self {
        Self {
            file_id: file_id.into(),
            token_type: ImageTokenType::Crop,
            bbox: Some(bbox),
            viewer_id: viewer,
        }
    }

    /// 加密头像缩略图 token
    ///
    /// # 参数
    /// - `cipher`: token 加密器
    /// - `avatar_file_id`: 头像文件 ID，为 `None` 时返回 `None`
    /// - `viewer`: 浏览者用户 ID
    ///
    /// # 返回
    /// 加密后的头像 token，加密失败返回 `None`
    #[cfg(feature = "orm")]
    pub fn encrypt_avatar_token(
        cipher: &TokenCipher,
        avatar_file_id: Option<&str>,
        viewer: UserId,
    ) -> Option<String> {
        avatar_file_id.and_then(|key| {
            let seed = format!("{}:{}", viewer, key);
            cipher
                .encrypt(&Self::thumbnail(viewer, key), Some(&seed))
                .trace_warn("encrypt_avatar_token_err", "加密头像失败", AppError::Ignore)
                .ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumbnail_constructor() {
        let token = ImageToken::thumbnail(UserId(7), "abc123");
        assert_eq!(token.file_id, "abc123");
        assert_eq!(token.token_type, ImageTokenType::Thumbnail);
        assert!(token.bbox.is_none());
        assert_eq!(token.viewer_id, UserId(7));
    }

    #[test]
    fn test_thumbnail_accepts_string() {
        let token = ImageToken::thumbnail(UserId(1), String::from("file-001"));
        assert_eq!(token.file_id, "file-001");
        assert_eq!(token.token_type, ImageTokenType::Thumbnail);
    }

    #[test]
    fn test_preview_constructor() {
        let token = ImageToken::preview(UserId(2), "preview-id");
        assert_eq!(token.file_id, "preview-id");
        assert_eq!(token.token_type, ImageTokenType::Preview);
        assert!(token.bbox.is_none());
    }

    #[test]
    fn test_original_constructor() {
        let token = ImageToken::original(UserId(3), "original-id");
        assert_eq!(token.file_id, "original-id");
        assert_eq!(token.token_type, ImageTokenType::Original);
        assert!(token.bbox.is_none());
    }

    #[test]
    fn test_crop_with_bbox() {
        let bbox = FaceBBox {
            x1: 0.1,
            y1: 0.2,
            x2: 0.6,
            y2: 0.9,
        };
        let token = ImageToken::crop(UserId(4), "crop-id", bbox);
        assert_eq!(token.file_id, "crop-id");
        assert_eq!(token.token_type, ImageTokenType::Crop);
        let b = token.bbox.unwrap();
        assert_eq!(b.x1, 0.1);
        assert_eq!(b.y1, 0.2);
        assert_eq!(b.x2, 0.6);
        assert_eq!(b.y2, 0.9);
    }

    #[test]
    fn test_image_token_type_serialize() {
        assert_eq!(
            serde_json::to_string(&ImageTokenType::Thumbnail).unwrap(),
            "\"thumbnail\""
        );
        assert_eq!(
            serde_json::to_string(&ImageTokenType::Preview).unwrap(),
            "\"preview\""
        );
        assert_eq!(
            serde_json::to_string(&ImageTokenType::Original).unwrap(),
            "\"original\""
        );
        assert_eq!(
            serde_json::to_string(&ImageTokenType::Crop).unwrap(),
            "\"crop\""
        );
    }

    #[test]
    fn test_image_token_type_deserialize() {
        assert_eq!(
            serde_json::from_str::<ImageTokenType>("\"thumbnail\"").unwrap(),
            ImageTokenType::Thumbnail
        );
        assert_eq!(
            serde_json::from_str::<ImageTokenType>("\"preview\"").unwrap(),
            ImageTokenType::Preview
        );
        assert_eq!(
            serde_json::from_str::<ImageTokenType>("\"original\"").unwrap(),
            ImageTokenType::Original
        );
        assert_eq!(
            serde_json::from_str::<ImageTokenType>("\"crop\"").unwrap(),
            ImageTokenType::Crop
        );
    }

    #[test]
    fn test_image_token_type_invalid_value() {
        let result = serde_json::from_str::<ImageTokenType>("\"unknown\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_image_token_serialize_roundtrip() {
        let token = ImageToken::thumbnail(UserId(11), "file-abc");
        let json = serde_json::to_string(&token).unwrap();
        let deserialized: ImageToken = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_id, "file-abc");
        assert_eq!(deserialized.token_type, ImageTokenType::Thumbnail);
        assert!(deserialized.bbox.is_none());
        assert_eq!(deserialized.viewer_id, UserId(11));
    }

    #[test]
    fn test_image_token_crop_serialize_roundtrip() {
        let bbox = FaceBBox {
            x1: 0.05,
            y1: 0.1,
            x2: 0.55,
            y2: 0.7,
        };
        let token = ImageToken::crop(UserId(12), "file-xyz", bbox);
        let json = serde_json::to_string(&token).unwrap();
        let deserialized: ImageToken = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_id, "file-xyz");
        assert_eq!(deserialized.token_type, ImageTokenType::Crop);
        assert_eq!(deserialized.viewer_id, UserId(12));
        let b = deserialized.bbox.unwrap();
        assert_eq!(b.x1, 0.05);
        assert_eq!(b.y1, 0.1);
        assert_eq!(b.x2, 0.55);
        assert_eq!(b.y2, 0.7);
    }

    #[test]
    fn test_image_token_missing_viewer_fails_deserialize() {
        let json = r#"{"fileId":"file-abc","type":"preview"}"#;
        let result: Result<ImageToken, _> = serde_json::from_str(json);
        assert!(result.is_err(), "缺 viewerId 的旧 token 应反序列化失败");
    }

    #[test]
    fn test_image_token_json_uses_type_field() {
        let token = ImageToken::preview(UserId(5), "img-1");
        let json = serde_json::to_value(&token).unwrap();
        assert!(json.get("type").is_some());
        assert_eq!(json["type"], "preview");
    }

    #[test]
    fn test_image_token_bbox_omitted_when_none() {
        let token = ImageToken::original(UserId(6), "img-2");
        let json = serde_json::to_value(&token).unwrap();
        assert!(json.get("bbox").is_none());
    }

    #[test]
    fn test_face_bbox_to_pixel_rect() {
        let bbox = FaceBBox {
            x1: 0.1,
            y1: 0.25,
            x2: 0.6,
            y2: 1.0,
        };
        // 800x400 图片：x1=80, y1=100, x2=480, y2=400
        let (x, y, w, h) = bbox.to_pixel_rect(800, 400);
        assert_eq!(x, 80);
        assert_eq!(y, 100);
        assert_eq!(w, 400);
        assert_eq!(h, 300);
    }

    #[test]
    fn test_face_bbox_to_pixel_rect_clamps_bounds() {
        let bbox = FaceBBox {
            x1: -0.5,
            y1: 0.0,
            x2: 1.5,
            y2: 1.0,
        };
        let (x, y, w, h) = bbox.to_pixel_rect(200, 100);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
        assert_eq!(w, 200);
        assert_eq!(h, 100);
    }
}

#[cfg(all(test, feature = "orm"))]
mod orm_tests {
    use super::*;
    use common::utils::TokenCipher;

    fn test_cipher() -> TokenCipher {
        TokenCipher::new("test-key-for-unit-tests", "test-salt")
    }

    #[test]
    fn test_encrypt_avatar_token_some() {
        let cipher = test_cipher();
        let token = ImageToken::encrypt_avatar_token(&cipher, Some("avatar-file-id"), UserId(9));
        assert!(token.is_some());
        // 验证能解密回来
        let decrypted: ImageToken = cipher.decrypt(&token.unwrap()).unwrap();
        assert_eq!(decrypted.file_id, "avatar-file-id");
        assert_eq!(decrypted.token_type, ImageTokenType::Thumbnail);
        assert_eq!(decrypted.viewer_id, UserId(9));
    }

    #[test]
    fn test_encrypt_avatar_token_none() {
        let cipher = test_cipher();
        let token = ImageToken::encrypt_avatar_token(&cipher, None, UserId(9));
        assert!(token.is_none());
    }
}
