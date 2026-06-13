use serde::{Deserialize, Serialize};

/// 图片类型
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ImageTokenType {
    Thumbnail,
    Preview,
    Original,
    Crop,
}

/// 人脸边界框（绝对像素坐标）
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct FaceBBoxPixels {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 统一图片 Token
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImageToken {
    /// 文件路径
    pub file_id: String,
    /// 图片类型
    #[serde(rename = "type")]
    pub token_type: ImageTokenType,
    /// 人脸边界框（仅 Crop 类型需要）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<FaceBBoxPixels>,
}

impl ImageToken {
    /// 创建缩略图 token
    ///
    /// # 参数
    /// - `file_id`: 图片文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Thumbnail` 的 `ImageToken`
    pub fn thumbnail(file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            token_type: ImageTokenType::Thumbnail,
            bbox: None,
        }
    }

    /// 创建预览图 token
    ///
    /// # 参数
    /// - `file_id`: 图片文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Preview` 的 `ImageToken`
    pub fn preview(file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            token_type: ImageTokenType::Preview,
            bbox: None,
        }
    }

    /// 创建原图 token
    ///
    /// # 参数
    /// - `file_id`: 图片文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Original` 的 `ImageToken`
    pub fn original(file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            token_type: ImageTokenType::Original,
            bbox: None,
        }
    }

    /// 创建裁剪图 token（人脸封面）
    ///
    /// # 参数
    /// - `file_id`: 图片文件 ID
    /// - `bbox`: 人脸边界框，用于定位裁剪区域
    ///
    /// # 返回
    /// 返回类型为 `Crop` 且包含 `bbox` 的 `ImageToken`
    pub fn crop(file_id: impl Into<String>, bbox: FaceBBoxPixels) -> Self {
        Self {
            file_id: file_id.into(),
            token_type: ImageTokenType::Crop,
            bbox: Some(bbox),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumbnail_constructor() {
        let token = ImageToken::thumbnail("abc123");
        assert_eq!(token.file_id, "abc123");
        assert_eq!(token.token_type, ImageTokenType::Thumbnail);
        assert!(token.bbox.is_none());
    }

    #[test]
    fn test_thumbnail_accepts_string() {
        let token = ImageToken::thumbnail(String::from("file-001"));
        assert_eq!(token.file_id, "file-001");
        assert_eq!(token.token_type, ImageTokenType::Thumbnail);
    }

    #[test]
    fn test_preview_constructor() {
        let token = ImageToken::preview("preview-id");
        assert_eq!(token.file_id, "preview-id");
        assert_eq!(token.token_type, ImageTokenType::Preview);
        assert!(token.bbox.is_none());
    }

    #[test]
    fn test_original_constructor() {
        let token = ImageToken::original("original-id");
        assert_eq!(token.file_id, "original-id");
        assert_eq!(token.token_type, ImageTokenType::Original);
        assert!(token.bbox.is_none());
    }

    #[test]
    fn test_crop_with_bbox() {
        let bbox = FaceBBoxPixels {
            x: 10,
            y: 20,
            w: 100,
            h: 120,
        };
        let token = ImageToken::crop("crop-id", bbox);
        assert_eq!(token.file_id, "crop-id");
        assert_eq!(token.token_type, ImageTokenType::Crop);
        let b = token.bbox.unwrap();
        assert_eq!(b.x, 10);
        assert_eq!(b.y, 20);
        assert_eq!(b.w, 100);
        assert_eq!(b.h, 120);
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
        let token = ImageToken::thumbnail("file-abc");
        let json = serde_json::to_string(&token).unwrap();
        let deserialized: ImageToken = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_id, "file-abc");
        assert_eq!(deserialized.token_type, ImageTokenType::Thumbnail);
        assert!(deserialized.bbox.is_none());
    }

    #[test]
    fn test_image_token_crop_serialize_roundtrip() {
        let bbox = FaceBBoxPixels {
            x: 5,
            y: 10,
            w: 50,
            h: 60,
        };
        let token = ImageToken::crop("file-xyz", bbox);
        let json = serde_json::to_string(&token).unwrap();
        let deserialized: ImageToken = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_id, "file-xyz");
        assert_eq!(deserialized.token_type, ImageTokenType::Crop);
        let b = deserialized.bbox.unwrap();
        assert_eq!(b.x, 5);
        assert_eq!(b.y, 10);
        assert_eq!(b.w, 50);
        assert_eq!(b.h, 60);
    }

    #[test]
    fn test_image_token_json_uses_type_field() {
        let token = ImageToken::preview("img-1");
        let json = serde_json::to_value(&token).unwrap();
        assert!(json.get("type").is_some());
        assert_eq!(json["type"], "preview");
    }

    #[test]
    fn test_image_token_bbox_omitted_when_none() {
        let token = ImageToken::original("img-2");
        let json = serde_json::to_value(&token).unwrap();
        assert!(json.get("bbox").is_none());
    }

    #[test]
    fn test_face_bbox_pixels_creation() {
        let bbox = FaceBBoxPixels {
            x: 0,
            y: 0,
            w: 200,
            h: 200,
        };
        assert_eq!(bbox.x, 0);
        assert_eq!(bbox.y, 0);
        assert_eq!(bbox.w, 200);
        assert_eq!(bbox.h, 200);
    }
}
